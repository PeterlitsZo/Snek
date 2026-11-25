use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsString, JsValue, NativeFunction,
    Source,
    builtins::promise::PromiseState,
    object::{FunctionObjectBuilder, builtins::JsPromise},
};
use boa_gc::{Finalize, GcRefCell, Trace};

// parse() is synchronous, so the host checks automatic actions after draining jobs.
#[derive(Default, Trace, Finalize, JsData)]
struct Actions(GcRefCell<Vec<JsPromise>>);

pub(in crate::runtime) fn check_actions(context: &Context) -> JsResult<()> {
    if let Some(actions) = context.get_data::<Actions>() {
        for promise in actions.0.borrow().iter() {
            match promise.state() {
                PromiseState::Fulfilled(_) => {}
                PromiseState::Rejected(error) => return Err(JsError::from_opaque(error)),
                PromiseState::Pending => {
                    return Err(JsNativeError::error()
                        .with_message("command action did not complete: no pending tasks remain")
                        .into());
                }
            }
        }
    }
    Ok(())
}

pub(in crate::runtime) fn constructor(
    argv: JsValue,
    script: JsString,
    context: &mut Context,
) -> JsResult<JsValue> {
    if context.get_data::<Actions>().is_none() {
        context.insert_data(Actions::default());
    }
    let track = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|_, args, context| {
            let promise = JsPromise::resolve(args.get_or_undefined(0).clone(), context)?;
            context
                .get_data::<Actions>()
                .expect("action tracker exists")
                .0
                .borrow_mut()
                .push(promise);
            Ok(JsValue::undefined())
        }),
    )
    .build();
    // The embedded sources share a private factory scope, not the script's globals.
    let source = concat!(
        "(argv, script, trackAction) => {\n",
        include_str!("options.js"),
        include_str!("commands.js"),
        include_str!("parser.js"),
        include_str!("help.js"),
        include_str!("cli.js"),
        "\n}",
    );
    let factory = context.eval(Source::from_bytes(source))?;
    factory
        .as_callable()
        .expect("Cli factory is callable")
        .call(
            &JsValue::undefined(),
            &[argv, script.into(), track.into()],
            context,
        )
}
