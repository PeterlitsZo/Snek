mod command;
mod environment;
mod line_process;
mod lines;
mod output;
mod process;
mod template;

use boa_engine::{Context, JsNativeError, JsResult, JsValue};

pub(in crate::runtime) use environment::ShellEnvironment;

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
    let environment = context
        .get_data::<ShellEnvironment>()
        .expect("shell environment initialized with the runtime")
        .clone();
    command::create(input, environment, context)
}
