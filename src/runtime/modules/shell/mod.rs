mod command;
mod output;
mod process;
mod template;

use boa_engine::{Context, JsNativeError, JsResult, JsValue};

fn shell_error(message: impl Into<String>) -> boa_engine::JsError {
    JsNativeError::error().with_message(message.into()).into()
}

/// Create a lazy command from a tagged template.
pub(in crate::runtime) fn dollar(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let input = template::template_shell(args, context);
    command::create(input, context)
}
