use std::path::Path;

use boa_engine::{
    Context, JsArgs, JsNativeError, JsResult, JsValue, NativeFunction, Source, js_string,
    object::{ObjectInitializer, builtins::JsArray},
    property::Attribute,
};

use crate::runtime::state::RuntimeState;

fn string(value: &JsValue) -> JsResult<String> {
    value
        .as_string()
        .ok_or_else(|| JsNativeError::typ().with_message("expected a string"))?
        .to_std_string()
        .map_err(|_| {
            JsNativeError::typ()
                .with_message("expected valid Unicode")
                .into()
        })
}

fn exit_code(value: &JsValue) -> JsResult<u8> {
    value
        .as_number()
        .filter(|code| code.is_finite() && code.fract() == 0.0 && (0.0..=255.0).contains(code))
        .map(|code| code as u8)
        .ok_or_else(|| {
            JsNativeError::range()
                .with_message("exitCode must be an integer from 0 to 255")
                .into()
        })
}

fn host(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let operation = string(args.get_or_undefined(0))?;
    let state = context
        .get_data::<RuntimeState>()
        .expect("runtime state initialized")
        .clone();
    let mut state = state
        .0
        .lock()
        .map_err(|_| JsNativeError::error().with_message("process state lock poisoned"))?;
    match operation.as_str() {
        "cwd" => Ok(js_string!(state.cwd.to_str().ok_or_else(|| {
            JsNativeError::error().with_message("working directory is not valid Unicode")
        })?)
        .into()),
        "chdir" => {
            let path = string(args.get_or_undefined(1))?;
            if path.is_empty() || path.contains('\0') {
                return Err(JsNativeError::typ()
                    .with_message("invalid directory path")
                    .into());
            }
            state
                .chdir(Path::new(&path))
                .map_err(|error| JsNativeError::error().with_message(error.to_string()))?;
            Ok(JsValue::undefined())
        }
        "exitCode" => {
            if args.len() > 1 {
                state.exit_code = exit_code(&args[1])?;
            }
            Ok(state.exit_code.into())
        }
        "exit" => {
            let value = args.get_or_undefined(1);
            let code = if value.is_undefined() {
                state.exit_code
            } else {
                exit_code(value)?
            };
            drop(state);
            std::process::exit(i32::from(code));
        }
        "keys" => Ok(JsArray::from_iter(
            state.variables.iter().filter_map(|(key, value)| {
                Some(JsValue::from(js_string!(
                    key.to_str().filter(|_| value.to_str().is_some())?
                )))
            }),
            context,
        )
        .into()),
        "get" | "set" | "delete" => {
            let name = string(args.get_or_undefined(1))?;
            if name.is_empty() || name.contains(['=', '\0']) {
                return Err(JsNativeError::typ()
                    .with_message("invalid environment variable name")
                    .into());
            }
            let key = state
                .variables
                .keys()
                .find(|key| {
                    if cfg!(windows) {
                        key.to_str()
                            .is_some_and(|key| key.eq_ignore_ascii_case(&name))
                    } else {
                        *key == std::ffi::OsStr::new(&name)
                    }
                })
                .cloned()
                .unwrap_or_else(|| name.into());
            if operation == "get" {
                return Ok(state
                    .variables
                    .get(&key)
                    .and_then(|value| value.to_str())
                    .map(|value| js_string!(value).into())
                    .unwrap_or_default());
            }
            if operation == "delete" {
                state.variables.remove(&key);
            } else {
                let value = string(args.get_or_undefined(2))?;
                if value.contains('\0') {
                    return Err(JsNativeError::typ()
                        .with_message("environment values cannot contain NUL")
                        .into());
                }
                state.variables.insert(key, value.into());
            }
            Ok(JsValue::undefined())
        }
        "metadata" => {
            let executable = std::env::current_exe()
                .map_err(|error| JsNativeError::error().with_message(error.to_string()))?;
            let platform = match std::env::consts::OS {
                "macos" => "darwin",
                "windows" => "win32",
                other => other,
            };
            let arch = match std::env::consts::ARCH {
                "x86_64" => "x64",
                "aarch64" => "arm64",
                "x86" => "ia32",
                other => other,
            };
            Ok(ObjectInitializer::new(context)
                .property(
                    js_string!("execPath"),
                    js_string!(executable.to_string_lossy()),
                    Attribute::all(),
                )
                .property(js_string!("pid"), std::process::id(), Attribute::all())
                .property(
                    js_string!("platform"),
                    js_string!(platform),
                    Attribute::all(),
                )
                .property(js_string!("arch"), js_string!(arch), Attribute::all())
                .property(
                    js_string!("version"),
                    js_string!(concat!("v", env!("CARGO_PKG_VERSION"))),
                    Attribute::all(),
                )
                .build()
                .into())
        }
        _ => unreachable!(),
    }
}

pub(in crate::runtime) fn create(argv: JsValue, context: &mut Context) -> JsResult<JsValue> {
    let host = NativeFunction::from_fn_ptr(host).to_js_function(context.realm());
    let factory = context.eval(Source::from_bytes(include_str!("process.js")))?;
    factory
        .as_callable()
        .expect("process factory is callable")
        .call(&JsValue::undefined(), &[argv, host.into()], context)
}
