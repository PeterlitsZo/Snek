use boa_engine::{
    Context, JsArgs, JsNativeError, JsResult, JsValue, NativeFunction, Source,
    object::{
        FunctionObjectBuilder,
        builtins::{JsArrayBuffer, JsUint8Array},
    },
};

use super::binary::{array_buffer, view_bytes};

/// Build an isolated Uint8Array subclass without modifying the built-in constructor.
pub(in crate::runtime) fn constructor(context: &mut Context) -> JsResult<JsValue> {
    let normalize =
        FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(normalize))
            .length(1)
            .build();
    let factory = context.eval(Source::from_bytes(indoc::indoc! { r#"
        (normalize) => {
            return class Buffer extends Uint8Array {
                static from(value) {
                    return new Buffer(normalize(value));
                }
            };
        }
    "#, }))?;
    factory
        .as_callable()
        .expect("Buffer factory is callable")
        .call(&JsValue::undefined(), &[normalize.into()], context)
}

fn normalize(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let value = args.get_or_undefined(0);
    if let Some(text) = value.as_string() {
        return array_buffer(text.to_std_string_lossy().as_bytes(), context);
    }
    if let Some(object) = value.as_object() {
        if let Ok(buffer) = JsArrayBuffer::from_object(object.clone()) {
            if buffer.data().is_none() {
                return Err(JsNativeError::typ()
                    .with_message("ArrayBuffer is detached")
                    .into());
            }
            // Like Node Buffer.from(ArrayBuffer), share the supplied backing memory.
            return Ok(buffer.into());
        }
        if let Ok(view) = JsUint8Array::from_object(object) {
            return array_buffer(&view_bytes(&view, context)?, context);
        }
    }
    Err(JsNativeError::typ()
        .with_message("Buffer.from expects a string, ArrayBuffer, or Uint8Array")
        .into())
}
