use boa_engine::{Context, JsArgs, JsData, JsNativeError, JsObject, JsResult, JsValue, js_string};
use boa_gc::{Finalize, Trace};

/// Private immutable bytes shared by all readers of one command.
#[derive(Trace, Finalize, JsData)]
struct CapturedOutput {
    bytes: Vec<u8>,
}

/// Wrap stdout bytes as the private result of the shared execution promise.
pub(super) fn captured(bytes: Vec<u8>) -> JsValue {
    JsObject::from_proto_and_data(None, CapturedOutput { bytes }).into()
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
    Ok(js_string!(String::from_utf8_lossy(&output.bytes).as_ref()).into())
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
    let bytes = output.bytes.clone();
    drop(output);
    super::super::blob::from_bytes(bytes, context)
}
