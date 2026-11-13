mod transport;

use std::rc::Rc;

use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
    object::builtins::{JsPromise, JsUint8Array},
};
use boa_gc::{Finalize, Trace};

use super::binary;
use transport::{Config, Transport};

#[derive(Trace, Finalize, JsData)]
struct S3Client {
    // The transport contains only Rust data and never owns JavaScript values.
    #[unsafe_ignore_trace]
    transport: Rc<Transport>,
}

#[derive(Clone, Trace, Finalize, JsData)]
struct S3File {
    #[unsafe_ignore_trace]
    transport: Rc<Transport>,
    key: String,
}

pub(in crate::runtime) fn constructor(context: &mut Context) -> JsResult<JsValue> {
    register::<S3File>(context)?;
    register::<S3Client>(context)
}

fn register<C: Class>(context: &mut Context) -> JsResult<JsValue> {
    let mut builder = ClassBuilder::new::<C>(context);
    C::init(&mut builder)?;
    let class = builder.build();
    let constructor = class.constructor();
    context.realm().register_class::<C>(class);
    Ok(constructor.into())
}

fn type_error(message: impl Into<String>) -> JsError {
    JsNativeError::typ().with_message(message.into()).into()
}

fn string(value: &JsValue, name: &str) -> JsResult<String> {
    value
        .as_string()
        .ok_or_else(|| type_error(format!("{name} must be a string")))?
        .to_std_string()
        .map_err(|_| type_error(format!("{name} must contain valid Unicode")))
}

fn options(value: &JsValue) -> JsResult<JsObject> {
    value
        .as_object()
        .ok_or_else(|| type_error("options must be an object"))
}

fn optional_string(
    object: &JsObject,
    name: &str,
    context: &mut Context,
) -> JsResult<Option<String>> {
    let value = object.get(js_string!(name), context)?;
    if value.is_undefined() {
        Ok(None)
    } else {
        string(&value, name).map(Some)
    }
}

fn key(value: &JsValue) -> JsResult<String> {
    let key = string(value, "key")?;
    if key.is_empty() || key.len() > 1024 {
        return Err(type_error("key must contain 1 to 1024 UTF-8 bytes"));
    }
    Ok(key)
}

fn client(this: &JsValue) -> JsResult<Rc<Transport>> {
    let object = this
        .as_object()
        .ok_or_else(|| type_error("invalid S3Client receiver"))?;
    let client = object
        .downcast_ref::<S3Client>()
        .ok_or_else(|| type_error("invalid S3Client receiver"))?;
    Ok(client.transport.clone())
}

fn file(this: &JsValue) -> JsResult<S3File> {
    let object = this
        .as_object()
        .ok_or_else(|| type_error("invalid S3File receiver"))?;
    let file = object
        .downcast_ref::<S3File>()
        .ok_or_else(|| type_error("invalid S3File receiver"))?;
    Ok(file.clone())
}

fn service_error(error: transport::Error, context: &mut Context) -> JsResult<JsError> {
    let object = JsNativeError::error()
        .with_message(error.message)
        .into_opaque(context);
    object.set(js_string!("name"), js_string!("S3Error"), true, context)?;
    if let Some(status) = error.status {
        object.set(js_string!("status"), status, true, context)?;
    }
    if let Some(code) = error.code {
        object.set(js_string!("code"), js_string!(code), true, context)?;
    }
    if let Some(request_id) = error.request_id {
        object.set(
            js_string!("requestId"),
            js_string!(request_id),
            true,
            context,
        )?;
    }
    Ok(JsError::from_opaque(object.into()))
}

impl Class for S3Client {
    const NAME: &'static str = "S3Client";
    const LENGTH: usize = 1;

    fn data_constructor(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<Self> {
        let options = options(args.get_or_undefined(0))?;
        let access_key = string(
            &options.get(js_string!("accessKeyId"), context)?,
            "accessKeyId",
        )?;
        let secret_key = string(
            &options.get(js_string!("secretAccessKey"), context)?,
            "secretAccessKey",
        )?;
        let bucket = string(&options.get(js_string!("bucket"), context)?, "bucket")?;
        let region =
            optional_string(&options, "region", context)?.unwrap_or_else(|| "us-east-1".into());
        let endpoint = optional_string(&options, "endpoint", context)?
            .unwrap_or_else(|| format!("https://s3.{region}.amazonaws.com"));
        let session_token = optional_string(&options, "sessionToken", context)?;
        let transport = Transport::new(Config {
            access_key,
            secret_key,
            session_token,
            bucket,
            region,
            endpoint,
        })
        .map_err(type_error)?;
        Ok(Self {
            transport: Rc::new(transport),
        })
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        class.method(
            js_string!("file"),
            1,
            NativeFunction::from_fn_ptr(|this, args, context| {
                let transport = client(this)?;
                let key = key(args.get_or_undefined(0))?;
                Ok(S3File::from_data(S3File { transport, key }, context)?.into())
            }),
        );
        class.method(
            js_string!("presign"),
            1,
            NativeFunction::from_fn_ptr(|this, args, context| {
                let transport = client(this)?;
                let key = key(args.get_or_undefined(0))?;
                let mut method = "GET".to_owned();
                let mut expires = 86400;
                if !args.get_or_undefined(1).is_undefined() {
                    let options = options(args.get_or_undefined(1))?;
                    method = optional_string(&options, "method", context)?.unwrap_or(method);
                    let value = options.get(js_string!("expiresIn"), context)?;
                    if !value.is_undefined() {
                        let n = value.as_number().ok_or_else(|| {
                            type_error("expiresIn must be an integer number of seconds")
                        })?;
                        if !n.is_finite() || n.fract() != 0.0 || !(1.0..=604800.0).contains(&n) {
                            return Err(JsNativeError::range()
                                .with_message("expiresIn must be an integer from 1 to 604800")
                                .into());
                        }
                        expires = n as u64;
                    }
                }
                if !matches!(method.as_str(), "GET" | "PUT") {
                    return Err(type_error("presign method must be GET or PUT"));
                }
                match transport.presign(&key, &method, expires) {
                    Ok(url) => Ok(js_string!(url).into()),
                    Err(error) => Err(service_error(error, context)?),
                }
            }),
        );
        Ok(())
    }
}

impl Class for S3File {
    const NAME: &'static str = "S3File";

    fn data_constructor(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<Self> {
        Err(type_error("use S3Client.file() to create an S3File"))
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        class.method(
            js_string!("text"),
            0,
            NativeFunction::from_fn_ptr(|this, _, context| read(this, ReadAs::Text, context)),
        );
        class.method(
            js_string!("json"),
            0,
            NativeFunction::from_fn_ptr(|this, _, context| read(this, ReadAs::Json, context)),
        );
        class.method(
            js_string!("arrayBuffer"),
            0,
            NativeFunction::from_fn_ptr(|this, _, context| {
                read(this, ReadAs::ArrayBuffer, context)
            }),
        );
        class.method(js_string!("write"), 1, NativeFunction::from_fn_ptr(write));
        Ok(())
    }
}

enum ReadAs {
    Text,
    Json,
    ArrayBuffer,
}

fn read(this: &JsValue, format: ReadAs, context: &mut Context) -> JsResult<JsValue> {
    let file = match file(this) {
        Ok(file) => file,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    Ok(JsPromise::from_async_fn(
        async move |context| {
            let result = file.transport.get(&file.key).await;
            let context = &mut context.borrow_mut();
            let bytes = match result {
                Ok(bytes) => bytes,
                Err(error) => return Err(service_error(error, context)?),
            };
            match format {
                ReadAs::ArrayBuffer => binary::array_buffer(&bytes, context),
                ReadAs::Text => Ok(js_string!(String::from_utf8_lossy(&bytes).as_ref()).into()),
                ReadAs::Json => {
                    let text = js_string!(String::from_utf8_lossy(&bytes).as_ref());
                    let json = context.intrinsics().objects().json();
                    let parse = json.get(js_string!("parse"), context)?;
                    parse
                        .as_callable()
                        .ok_or_else(|| type_error("JSON.parse is not callable"))?
                        .call(&json.into(), &[text.into()], context)
                }
            }
        },
        context,
    )
    .into())
}

fn write(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    // All input reads and copies happen now, before the async job is queued.
    let input = (|| {
        let file = file(this)?;
        let data = args.get_or_undefined(0);
        let bytes = if let Some(text) = data.as_string() {
            text.to_std_string_lossy().into_bytes()
        } else {
            let object = data
                .as_object()
                .ok_or_else(|| type_error("write expects a string or Uint8Array"))?;
            let view = JsUint8Array::from_object(object)
                .map_err(|_| type_error("write expects a string or Uint8Array"))?;
            binary::view_bytes(&view, context)?
        };
        let content_type = if args.get_or_undefined(1).is_undefined() {
            None
        } else {
            optional_string(&options(args.get_or_undefined(1))?, "type", context)?
        }
        .unwrap_or_else(|| "application/octet-stream".into());
        if content_type.is_empty() || !content_type.bytes().all(|b| (32..=126).contains(&b)) {
            return Err(type_error(
                "type must be a nonempty ASCII HTTP header value",
            ));
        }
        Ok((file, bytes, content_type))
    })();
    let (file, bytes, content_type) = match input {
        Ok(input) => input,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    let length = bytes.len();
    Ok(JsPromise::from_async_fn(
        async move |context| match file
            .transport
            .put(&file.key, bytes.into(), &content_type)
            .await
        {
            Ok(()) => Ok(JsValue::from(length)),
            Err(error) => Err(service_error(error, &mut context.borrow_mut())?),
        },
        context,
    )
    .into())
}
