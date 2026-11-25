use std::{io, path::PathBuf};

use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
    object::builtins::{JsPromise, JsUint8Array},
};
use boa_gc::{Finalize, Trace};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{binary, blob};

#[derive(Clone, Trace, Finalize, JsData)]
struct LocalFile {
    #[unsafe_ignore_trace]
    path: PathBuf,
}

fn type_error(message: &'static str) -> JsError {
    JsNativeError::typ().with_message(message).into()
}

pub(in crate::runtime) fn file(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let path = args
        .get_or_undefined(0)
        .as_string()
        .ok_or_else(|| type_error("path must be a string"))?
        .to_std_string()
        .map_err(|_| type_error("path must contain valid Unicode"))?;
    if path.is_empty() || path.contains('\0') {
        return Err(type_error("path must be nonempty and contain no NUL"));
    }
    let path = std::path::absolute(path)
        .map_err(|error| JsNativeError::error().with_message(error.to_string()))?;
    if context.realm().get_class::<LocalFile>().is_none() {
        let mut builder = ClassBuilder::new::<LocalFile>(context);
        LocalFile::init(&mut builder)?;
        let class = builder.build();
        context.realm().register_class::<LocalFile>(class);
    }
    Ok(LocalFile::from_data(LocalFile { path }, context)?.into())
}

fn receiver(this: &JsValue) -> JsResult<LocalFile> {
    let object = this
        .as_object()
        .ok_or_else(|| type_error("invalid LocalFile receiver"))?;
    let file = object
        .downcast_ref::<LocalFile>()
        .ok_or_else(|| type_error("invalid LocalFile receiver"))?;
    Ok(file.clone())
}

fn file_error(
    error: io::Error,
    file: &LocalFile,
    operation: &str,
    context: &mut Context,
) -> JsResult<JsError> {
    let code = match error.kind() {
        io::ErrorKind::NotFound => "ENOENT",
        io::ErrorKind::PermissionDenied => "EACCES",
        io::ErrorKind::IsADirectory => "EISDIR",
        io::ErrorKind::NotADirectory => "ENOTDIR",
        io::ErrorKind::AlreadyExists => "EEXIST",
        io::ErrorKind::InvalidInput => "EINVAL",
        io::ErrorKind::Unsupported => "ENOTSUP",
        _ => "EIO",
    };
    let object = JsNativeError::error()
        .with_message(error.to_string())
        .into_opaque(context);
    for (key, value) in [
        ("name", "FileError"),
        ("code", code),
        ("operation", operation),
        ("path", file.path.to_string_lossy().as_ref()),
    ] {
        object.set(js_string!(key), js_string!(value), true, context)?;
    }
    Ok(JsError::from_opaque(object.into()))
}

fn regular(metadata: &std::fs::Metadata) -> io::Result<()> {
    if metadata.is_dir() {
        Err(io::Error::new(
            io::ErrorKind::IsADirectory,
            "path is a directory",
        ))
    } else if !metadata.is_file() {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "only regular files are supported",
        ))
    } else {
        Ok(())
    }
}

impl Class for LocalFile {
    const NAME: &'static str = "LocalFile";

    fn data_constructor(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<Self> {
        Err(type_error("use file() to create a LocalFile"))
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        class.method(
            js_string!("text"),
            0,
            NativeFunction::from_fn_ptr(|this, _, context| read(this, "text", context)),
        );
        class.method(
            js_string!("json"),
            0,
            NativeFunction::from_fn_ptr(|this, _, context| read(this, "json", context)),
        );
        class.method(
            js_string!("arrayBuffer"),
            0,
            NativeFunction::from_fn_ptr(|this, _, context| read(this, "arrayBuffer", context)),
        );
        class.method(
            js_string!("blob"),
            0,
            NativeFunction::from_fn_ptr(|this, _, context| read(this, "blob", context)),
        );
        class.method(
            js_string!("exists"),
            0,
            NativeFunction::from_fn_ptr(|this, _, context| read(this, "exists", context)),
        );

        class.method(
            js_string!("write"),
            1,
            NativeFunction::from_fn_ptr(|this, args, context| write(this, args, false, context)),
        );
        class.method(
            js_string!("append"),
            1,
            NativeFunction::from_fn_ptr(|this, args, context| write(this, args, true, context)),
        );
        Ok(())
    }
}

fn read(this: &JsValue, operation: &'static str, context: &mut Context) -> JsResult<JsValue> {
    let file = match receiver(this) {
        Ok(file) => file,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    Ok(JsPromise::from_async_fn(
        async move |context| {
            let result = async {
                let metadata = tokio::fs::metadata(&file.path).await?;
                regular(&metadata)?;
                if operation == "exists" {
                    return Ok(Vec::new());
                }
                let mut handle = tokio::fs::File::open(&file.path).await?;
                regular(&handle.metadata().await?)?;
                let mut bytes = Vec::new();
                handle.read_to_end(&mut bytes).await?;
                Ok::<_, io::Error>(bytes)
            }
            .await;
            let context = &mut context.borrow_mut();
            let bytes = match result {
                Ok(bytes) => bytes,
                Err(error) if operation == "exists" && error.kind() == io::ErrorKind::NotFound => {
                    return Ok(false.into());
                }
                Err(error) => return Err(file_error(error, &file, operation, context)?),
            };
            match operation {
                "exists" => Ok(true.into()),
                "arrayBuffer" => binary::array_buffer(&bytes, context),
                "blob" => blob::from_bytes(bytes, context),
                "json" => {
                    let text = js_string!(String::from_utf8_lossy(&bytes).as_ref());
                    let json = context.intrinsics().objects().json();
                    let parse = json.get(js_string!("parse"), context)?;
                    parse
                        .as_callable()
                        .ok_or_else(|| type_error("JSON.parse is not callable"))?
                        .call(&json.into(), &[text.into()], context)
                }
                _ => Ok(js_string!(String::from_utf8_lossy(&bytes).as_ref()).into()),
            }
        },
        context,
    )
    .into())
}

fn write(
    this: &JsValue,
    args: &[JsValue],
    append: bool,
    context: &mut Context,
) -> JsResult<JsValue> {
    let input = (|| {
        let file = receiver(this)?;
        let data = args.get_or_undefined(0);
        let bytes = if let Some(text) = data.as_string() {
            text.to_std_string_lossy().into_bytes()
        } else {
            let view = data
                .as_object()
                .and_then(|object| JsUint8Array::from_object(object).ok())
                .ok_or_else(|| type_error("write expects a string or Uint8Array"))?;
            binary::view_bytes(&view, context)?
        };
        Ok::<_, JsError>((file, bytes))
    })();
    let (file, bytes) = match input {
        Ok(input) => input,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    Ok(JsPromise::from_async_fn(
        async move |context| {
            let result = async {
                match tokio::fs::metadata(&file.path).await {
                    Ok(metadata) => regular(&metadata)?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
                let mut handle = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(append)
                    .open(&file.path)
                    .await?;
                regular(&handle.metadata().await?)?;
                if !append {
                    handle.set_len(0).await?;
                }
                handle.write_all(&bytes).await?;
                handle.flush().await
            }
            .await;
            match result {
                Ok(()) => Ok(JsValue::from(bytes.len())),
                Err(error) => Err(file_error(
                    error,
                    &file,
                    if append { "append" } else { "write" },
                    &mut context.borrow_mut(),
                )?),
            }
        },
        context,
    )
    .into())
}
