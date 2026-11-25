use boa_engine::{
    Context, JsArgs, JsData, JsNativeError, JsObject, JsResult, JsValue, js_string,
    object::ObjectInitializer, property::Attribute,
};
use boa_gc::{Finalize, Trace};

use super::{process::ProcessOutput, shell_error};

/// Private immutable execution result shared by all readers of one command.
#[derive(Trace, Finalize, JsData)]
struct CapturedOutput {
    #[unsafe_ignore_trace]
    result: ProcessOutput,
}

/// Wrap the process result for the shared execution promise.
pub(super) fn captured(result: ProcessOutput) -> JsValue {
    JsObject::from_proto_and_data(None, CapturedOutput { result }).into()
}

/// Decode the captured result in args[0] as UTF-8, replacing invalid sequences.
pub(super) fn text(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let object = args
        .get_or_undefined(0)
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid captured shell output"))?;
    let output = object
        .downcast_ref::<CapturedOutput>()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid captured shell output"))?;
    Ok(js_string!(String::from_utf8_lossy(&output.result.stdout).as_ref()).into())
}

/// Parse decoded stdout through JSON.parse, preserving its parsing errors.
pub(super) fn json(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let text = text(this, args, context)?;
    let json = context.intrinsics().objects().json();
    let parse = json.get(js_string!("parse"), context)?;
    parse
        .as_callable()
        .ok_or_else(|| JsNativeError::typ().with_message("JSON.parse is not callable"))?
        .call(&json.into(), &[text], context)
}

/// Copy captured bytes into a Blob without text decoding or MIME type
/// inference.
pub(super) fn blob(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let object = args
        .get_or_undefined(0)
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid captured shell output"))?;
    let output = object
        .downcast_ref::<CapturedOutput>()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid captured shell output"))?;
    let bytes = output.result.stdout.clone();
    drop(output);
    super::super::blob::from_bytes(bytes, context)
}

/// Apply the default exit policy without changing the shared execution result.
pub(super) fn checked(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let value = args.get_or_undefined(0);
    let object = value
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid captured shell output"))?;
    let output = object
        .downcast_ref::<CapturedOutput>()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid captured shell output"))?;
    if let Some(message) = &output.result.failure {
        return Err(shell_error(message.clone()));
    }
    Ok(value.clone())
}

/// Return an independent public snapshot, including unsuccessful exits.
pub(super) fn nothrow(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let object = args
        .get_or_undefined(0)
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid captured shell output"))?;
    let output = object
        .downcast_ref::<CapturedOutput>()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid captured shell output"))?;
    let stdout = js_string!(String::from_utf8_lossy(&output.result.stdout).as_ref());
    let stderr = js_string!(String::from_utf8_lossy(&output.result.stderr).as_ref());
    let exit_code = output
        .result
        .exit_code
        .map(JsValue::from)
        .unwrap_or_else(JsValue::null);
    drop(output);
    Ok(ObjectInitializer::new(context)
        .property(js_string!("stdout"), stdout, Attribute::all())
        .property(js_string!("stderr"), stderr, Attribute::all())
        .property(js_string!("exitCode"), exit_code, Attribute::all())
        .build()
        .into())
}
