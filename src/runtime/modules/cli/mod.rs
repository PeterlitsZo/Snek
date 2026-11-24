use boa_engine::{Context, JsResult, JsString, JsValue, Source};

pub(in crate::runtime) fn constructor(
    argv: JsValue,
    script: JsString,
    context: &mut Context,
) -> JsResult<JsValue> {
    let factory = context.eval(Source::from_bytes(include_str!("cli.js")))?;
    factory
        .as_callable()
        .expect("Cli factory is callable")
        .call(&JsValue::undefined(), &[argv, script.into()], context)
}
