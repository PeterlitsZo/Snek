use std::rc::Rc;

use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
    object::builtins::{JsArray, JsArrayBuffer, JsPromise, JsUint8Array},
    property::Attribute,
};
use boa_gc::{Finalize, Trace};

use super::binary;

/// Immutable bytes with an optional normalized MIME type.
#[derive(Clone, Trace, Finalize, JsData)]
struct Blob {
    // Contains only Rust bytes, never JavaScript references.
    #[unsafe_ignore_trace]
    bytes: Rc<[u8]>,
    media_type: String,
}

fn type_error(message: &'static str) -> JsError {
    JsNativeError::typ().with_message(message).into()
}

fn receiver(this: &JsValue) -> JsResult<Blob> {
    let object = this
        .as_object()
        .ok_or_else(|| type_error("invalid Blob receiver"))?;
    let blob = object
        .downcast_ref::<Blob>()
        .ok_or_else(|| type_error("invalid Blob receiver"))?;
    Ok(blob.clone())
}

/// Register once so shell output and constructed Blobs share the same prototype.
pub(in crate::runtime) fn constructor(context: &mut Context) -> JsResult<JsValue> {
    if let Some(class) = context.realm().get_class::<Blob>() {
        return Ok(class.constructor().into());
    }
    let mut builder = ClassBuilder::new::<Blob>(context);
    Blob::init(&mut builder)?;
    let class = builder.build();
    let constructor = class.constructor();
    context.realm().register_class::<Blob>(class);
    Ok(constructor.into())
}

pub(super) fn from_bytes(bytes: Vec<u8>, context: &mut Context) -> JsResult<JsValue> {
    constructor(context)?;
    Ok(Blob::from_data(
        Blob {
            bytes: bytes.into(),
            media_type: String::new(),
        },
        context,
    )?
    .into())
}

/// MIME types retain printable ASCII only and are lowercased, without trimming.
fn normalize_type(value: &JsValue, context: &mut Context) -> JsResult<String> {
    if value.is_undefined() {
        return Ok(String::new());
    }
    let text = value.to_string(context)?.to_std_string_lossy();
    if text.bytes().any(|byte| !(0x20..=0x7e).contains(&byte)) {
        return Ok(String::new());
    }
    Ok(text.to_ascii_lowercase())
}

/// Copy mutable buffers and views, honoring their visible byte range.
fn part_bytes(value: &JsValue, context: &mut Context) -> JsResult<Vec<u8>> {
    if let Some(text) = value.as_string() {
        return Ok(text.to_std_string_lossy().into_bytes());
    }
    if let Some(object) = value.as_object() {
        if let Some(blob) = object.downcast_ref::<Blob>() {
            return Ok(blob.bytes.to_vec());
        }
        if let Ok(buffer) = JsArrayBuffer::from_object(object.clone()) {
            return buffer
                .data()
                .map(|bytes| bytes.to_vec())
                .ok_or_else(|| type_error("Blob part has a detached ArrayBuffer"));
        }
        if let Ok(view) = JsUint8Array::from_object(object) {
            return binary::view_bytes(&view, context);
        }
    }
    Err(type_error(
        "Blob parts must be strings, ArrayBuffers, Uint8Arrays, or Blobs",
    ))
}

impl Class for Blob {
    const NAME: &'static str = "Blob";
    const LENGTH: usize = 0;

    fn data_constructor(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<Self> {
        let mut bytes = Vec::new();
        let parts = args.get_or_undefined(0);
        if !parts.is_undefined() {
            let array = parts
                .as_object()
                .and_then(|object| JsArray::from_object(object).ok())
                .ok_or_else(|| type_error("Blob parts must be an array"))?;
            let length = array.length(context)?;
            for index in 0..length {
                let part = array.get(index, context)?;
                bytes.extend(part_bytes(&part, context)?);
            }
        }
        let options = args.get_or_undefined(1);
        let media_type = if options.is_null_or_undefined() {
            String::new()
        } else {
            let options = options
                .as_object()
                .ok_or_else(|| type_error("Blob options must be an object"))?;
            normalize_type(&options.get(js_string!("type"), context)?, context)?
        };
        Ok(Self {
            bytes: bytes.into(),
            media_type,
        })
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        let size = NativeFunction::from_fn_ptr(|this, _, _| {
            Ok((receiver(this)?.bytes.len() as f64).into())
        })
        .to_js_function(class.context().realm());
        class.accessor(
            js_string!("size"),
            Some(size),
            None,
            Attribute::CONFIGURABLE,
        );
        let media_type = NativeFunction::from_fn_ptr(|this, _, _| {
            Ok(js_string!(receiver(this)?.media_type.as_str()).into())
        })
        .to_js_function(class.context().realm());
        class.accessor(
            js_string!("type"),
            Some(media_type),
            None,
            Attribute::CONFIGURABLE,
        );
        class.method(js_string!("text"), 0, NativeFunction::from_fn_ptr(text));
        class.method(
            js_string!("arrayBuffer"),
            0,
            NativeFunction::from_fn_ptr(array_buffer),
        );
        class.method(js_string!("slice"), 0, NativeFunction::from_fn_ptr(slice));
        Ok(())
    }
}

fn text(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let result = receiver(this)
        .map(|blob| JsValue::from(js_string!(String::from_utf8_lossy(&blob.bytes).as_ref())));
    promise(result, context)
}

fn array_buffer(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let result = receiver(this).and_then(|blob| binary::array_buffer(&blob.bytes, context));
    promise(result, context)
}

fn promise(result: JsResult<JsValue>, context: &mut Context) -> JsResult<JsValue> {
    Ok(match result {
        Ok(value) => JsPromise::resolve(value, context)?,
        Err(error) => JsPromise::reject(error, context)?,
    }
    .into())
}

/// Clamp slice offsets after truncation; negative offsets count back from the end.
fn offset(
    value: &JsValue,
    default: usize,
    length: usize,
    context: &mut Context,
) -> JsResult<usize> {
    if value.is_undefined() {
        return Ok(default);
    }
    let number = value.to_number(context)?;
    let index = if number.is_nan() { 0.0 } else { number.trunc() };
    let index = if index < 0.0 {
        length as f64 + index
    } else {
        index
    };
    Ok(index.clamp(0.0, length as f64) as usize)
}

fn slice(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let blob = receiver(this)?;
    let length = blob.bytes.len();
    let start = offset(args.get_or_undefined(0), 0, length, context)?;
    let end = offset(args.get_or_undefined(1), length, length, context)?.max(start);
    let media_type = normalize_type(args.get_or_undefined(2), context)?;
    Ok(Blob::from_data(
        Blob {
            bytes: blob.bytes[start..end].into(),
            media_type,
        },
        context,
    )?
    .into())
}
