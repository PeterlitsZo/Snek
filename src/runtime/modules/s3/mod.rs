mod transport;

use std::rc::Rc;

use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
    object::builtins::{JsDate, JsPromise, JsUint8Array},
};
use boa_gc::{Finalize, Trace};

use super::{binary, blob};
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
            timeout_ms: timeout(&options.clone().into(), context)?.unwrap_or(60000),
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
                presign(
                    &S3File {
                        transport: client(this)?,
                        key: key(args.get_or_undefined(0))?,
                    },
                    args.get_or_undefined(1),
                    context,
                )
            }),
        );
        class.method(
            js_string!("write"),
            2,
            NativeFunction::from_fn_ptr(|this, args, context| {
                let target = (|| {
                    Ok(S3File {
                        transport: client(this)?,
                        key: key(args.get_or_undefined(0))?,
                    })
                })();
                operation(target, "write", &args[args.len().min(1)..], context)
            }),
        );
        class.method(
            js_string!("stat"),
            1,
            NativeFunction::from_fn_ptr(|this, args, context| {
                let target = (|| {
                    Ok(S3File {
                        transport: client(this)?,
                        key: key(args.get_or_undefined(0))?,
                    })
                })();
                operation(target, "stat", &args[args.len().min(1)..], context)
            }),
        );
        class.method(
            js_string!("exists"),
            1,
            NativeFunction::from_fn_ptr(|this, args, context| {
                let target = (|| {
                    Ok(S3File {
                        transport: client(this)?,
                        key: key(args.get_or_undefined(0))?,
                    })
                })();
                operation(target, "exists", &args[args.len().min(1)..], context)
            }),
        );
        class.method(
            js_string!("delete"),
            1,
            NativeFunction::from_fn_ptr(|this, args, context| {
                let target = (|| {
                    Ok(S3File {
                        transport: client(this)?,
                        key: key(args.get_or_undefined(0))?,
                    })
                })();
                operation(target, "delete", &args[args.len().min(1)..], context)
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
            NativeFunction::from_fn_ptr(|this, args, context| {
                operation(file(this), "text", args, context)
            }),
        );
        class.method(
            js_string!("json"),
            0,
            NativeFunction::from_fn_ptr(|this, args, context| {
                operation(file(this), "json", args, context)
            }),
        );
        class.method(
            js_string!("arrayBuffer"),
            0,
            NativeFunction::from_fn_ptr(|this, args, context| {
                operation(file(this), "arrayBuffer", args, context)
            }),
        );
        class.method(
            js_string!("bytes"),
            0,
            NativeFunction::from_fn_ptr(|this, args, context| {
                operation(file(this), "bytes", args, context)
            }),
        );
        class.method(
            js_string!("blob"),
            0,
            NativeFunction::from_fn_ptr(|this, args, context| {
                operation(file(this), "blob", args, context)
            }),
        );
        class.method(
            js_string!("write"),
            1,
            NativeFunction::from_fn_ptr(|this, args, context| {
                operation(file(this), "write", args, context)
            }),
        );
        class.method(
            js_string!("stat"),
            0,
            NativeFunction::from_fn_ptr(|this, args, context| {
                operation(file(this), "stat", args, context)
            }),
        );
        class.method(
            js_string!("exists"),
            0,
            NativeFunction::from_fn_ptr(|this, args, context| {
                operation(file(this), "exists", args, context)
            }),
        );
        class.method(
            js_string!("delete"),
            0,
            NativeFunction::from_fn_ptr(|this, args, context| {
                operation(file(this), "delete", args, context)
            }),
        );
        class.method(
            js_string!("presign"),
            0,
            NativeFunction::from_fn_ptr(|this, args, context| {
                presign(&file(this)?, args.get_or_undefined(0), context)
            }),
        );
        Ok(())
    }
}

fn timeout(value: &JsValue, context: &mut Context) -> JsResult<Option<u64>> {
    if value.is_undefined() {
        return Ok(None);
    }
    let value = options(value)?.get(js_string!("timeoutMs"), context)?;
    if value.is_undefined() {
        return Ok(None);
    }
    let n = value
        .as_number()
        .ok_or_else(|| type_error("timeoutMs must be a positive safe integer"))?;
    if !n.is_finite() || n.fract() != 0.0 || !(1.0..=9007199254740991.0).contains(&n) {
        return Err(type_error("timeoutMs must be a positive safe integer"));
    }
    Ok(Some(n as u64))
}

fn presign(file: &S3File, value: &JsValue, context: &mut Context) -> JsResult<JsValue> {
    let mut method = "GET".to_owned();
    let mut expires = 86400;
    if !value.is_undefined() {
        let opts = options(value)?;
        method = optional_string(&opts, "method", context)?.unwrap_or(method);
        let value = opts.get(js_string!("expiresIn"), context)?;
        if !value.is_undefined() {
            let n = value
                .as_number()
                .ok_or_else(|| type_error("expiresIn must be an integer"))?;
            if !n.is_finite() || n.fract() != 0.0 || !(1.0..=604800.0).contains(&n) {
                return Err(JsNativeError::range()
                    .with_message("expiresIn must be an integer from 1 to 604800")
                    .into());
            }
            expires = n as u64;
        }
    }
    if !matches!(method.as_str(), "GET" | "PUT" | "HEAD" | "DELETE") {
        return Err(type_error("invalid presign method"));
    }
    file.transport
        .presign(&file.key, &method, expires)
        .map(|url| js_string!(url).into())
        .map_err(|error| service_error(error, context).unwrap_or_else(|error| error))
}

fn write_headers(value: &JsValue, context: &mut Context) -> JsResult<Vec<(String, String)>> {
    let mut headers = vec![("content-type".into(), "application/octet-stream".into())];
    if value.is_undefined() {
        return Ok(headers);
    }
    let opts = options(value)?;
    for (name, header) in [
        ("type", "content-type"),
        ("ifNoneMatch", "if-none-match"),
        ("contentEncoding", "content-encoding"),
        ("contentDisposition", "content-disposition"),
        ("cacheControl", "cache-control"),
    ] {
        if let Some(value) = optional_string(&opts, name, context)? {
            if value.is_empty()
                || !value.bytes().all(|b| (32..=126).contains(&b))
                || (name == "ifNoneMatch" && value != "*")
            {
                return Err(type_error(format!("invalid {name}")));
            }
            headers.retain(|(key, _)| key != header);
            headers.push((header.into(), value));
        }
    }
    let metadata = opts.get(js_string!("metadata"), context)?;
    if !metadata.is_undefined() {
        let metadata = options(&metadata)?;
        let mut names = std::collections::BTreeSet::new();
        for property in metadata.own_property_keys(context)? {
            let name = match &property {
                boa_engine::property::PropertyKey::String(name) => name
                    .to_std_string()
                    .map_err(|_| type_error("invalid metadata key"))?,
                boa_engine::property::PropertyKey::Index(index) => index.get().to_string(),
                _ => return Err(type_error("metadata keys must be strings")),
            }
            .to_ascii_lowercase();
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
                || !names.insert(name.clone())
            {
                return Err(type_error("invalid or duplicate metadata key"));
            }
            let value = string(&metadata.get(property, context)?, "metadata value")?;
            if !value.bytes().all(|b| (32..=126).contains(&b)) {
                return Err(type_error("metadata values must be printable ASCII"));
            }
            headers.push((format!("x-amz-meta-{name}"), value));
        }
    }
    Ok(headers)
}

fn operation(
    target: JsResult<S3File>,
    operation: &'static str,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let input = (|| {
        let file = target?;
        let opts = args.get_or_undefined(if operation == "write" { 1 } else { 0 });
        let timeout = timeout(opts, context)?;
        let (bytes, headers) = if operation == "write" {
            (
                blob::part_bytes(args.get_or_undefined(0), context)?,
                write_headers(opts, context)?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        Ok::<_, JsError>((file, timeout, bytes, headers))
    })();
    let (file, timeout, bytes, headers) = match input {
        Ok(input) => input,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    let length = bytes.len();
    Ok(JsPromise::from_async_fn(
        async move |context| {
            let method = match operation {
                "write" => "PUT",
                "stat" | "exists" => "HEAD",
                "delete" => "DELETE",
                _ => "GET",
            };
            let result = file
                .transport
                .request(&file.key, method, bytes.into(), headers, timeout)
                .await;
            let context = &mut context.borrow_mut();
            let (headers, bytes) = match result {
                Ok(result) => result,
                Err(error)
                    if error.status == Some(404)
                        && error.message == "S3 request failed"
                        && matches!(operation, "exists" | "delete") =>
                {
                    return Ok(if operation == "exists" {
                        false.into()
                    } else {
                        JsValue::undefined()
                    });
                }
                Err(error) => {
                    let error = service_error(error, context)?;
                    let object = error
                        .clone()
                        .into_opaque(context)?
                        .as_object()
                        .expect("S3Error is an object");
                    object.set(
                        js_string!("operation"),
                        js_string!(operation),
                        true,
                        context,
                    )?;
                    object.set(
                        js_string!("key"),
                        js_string!(file.key.as_str()),
                        true,
                        context,
                    )?;
                    return Err(error);
                }
            };
            match operation {
                "write" => Ok(JsValue::from(length)),
                "delete" => Ok(JsValue::undefined()),
                "exists" => Ok(true.into()),
                "stat" => match stat(&headers, context) {
                    Ok(value) => Ok(value),
                    Err(_) => {
                        let mut error = transport::Error::new("invalid S3 HEAD response");
                        error.code = Some("ProtocolError".into());
                        error.status = Some(200);
                        error.request_id = headers
                            .get("x-amz-request-id")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned);
                        let error = service_error(error, context)?;
                        let object = error
                            .clone()
                            .into_opaque(context)?
                            .as_object()
                            .expect("S3Error is an object");
                        object.set(
                            js_string!("operation"),
                            js_string!(operation),
                            true,
                            context,
                        )?;
                        object.set(
                            js_string!("key"),
                            js_string!(file.key.as_str()),
                            true,
                            context,
                        )?;
                        Err(error)
                    }
                },
                "arrayBuffer" => binary::array_buffer(&bytes, context),
                "bytes" => Ok(JsUint8Array::from_iter(bytes.iter().copied(), context)?.into()),
                "blob" => blob::from_typed_bytes(
                    bytes.to_vec(),
                    headers
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_ascii_lowercase(),
                    context,
                ),
                "json" => {
                    let json = context.intrinsics().objects().json();
                    let parse = json.get(js_string!("parse"), context)?;
                    parse
                        .as_callable()
                        .ok_or_else(|| type_error("JSON.parse is not callable"))?
                        .call(
                            &json.into(),
                            &[js_string!(String::from_utf8_lossy(&bytes).as_ref()).into()],
                            context,
                        )
                }
                _ => Ok(js_string!(String::from_utf8_lossy(&bytes).as_ref()).into()),
            }
        },
        context,
    )
    .into())
}

fn stat(headers: &http::HeaderMap, context: &mut Context) -> JsResult<JsValue> {
    let invalid = || JsNativeError::error().with_message("invalid S3 HEAD response");
    let required = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(invalid)
    };
    let length = required("content-length")?;
    if length.is_empty() || !length.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid().into());
    }
    let size: u64 = length.parse().map_err(|_| invalid())?;
    if size > 9007199254740991 {
        return Err(invalid().into());
    }
    let etag = required("etag")?;
    if etag.len() < 2 || !etag.starts_with('"') || !etag.ends_with('"') {
        return Err(invalid().into());
    }
    let modified = httpdate::parse_http_date(required("last-modified")?).map_err(|_| invalid())?;
    let millis = match modified.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64() * 1000.0,
        Err(error) => -error.duration().as_secs_f64() * 1000.0,
    };
    let date = JsDate::new(context);
    date.set_time(millis, context)?;
    let result = JsObject::with_object_proto(context.intrinsics());
    result.set(js_string!("size"), size as f64, true, context)?;
    result.set(js_string!("etag"), js_string!(etag), true, context)?;
    result.set(js_string!("lastModified"), date, true, context)?;
    result.set(
        js_string!("type"),
        js_string!(
            headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
        ),
        true,
        context,
    )?;
    let metadata = JsObject::with_null_proto();
    for (name, value) in headers {
        if let Some(name) = name.as_str().strip_prefix("x-amz-meta-") {
            metadata.create_data_property_or_throw(
                js_string!(name),
                js_string!(value.to_str().map_err(|_| invalid())?),
                context,
            )?;
        }
    }
    result.set(js_string!("metadata"), metadata, true, context)?;
    for (header, name) in [
        ("content-encoding", "contentEncoding"),
        ("content-disposition", "contentDisposition"),
        ("cache-control", "cacheControl"),
    ] {
        if let Some(value) = headers.get(header) {
            result.set(
                js_string!(name),
                js_string!(value.to_str().map_err(|_| invalid())?),
                true,
                context,
            )?;
        }
    }
    Ok(result.into())
}
