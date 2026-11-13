use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
    object::builtins::{JsFunction, JsPromise},
};
use boa_gc::{Finalize, Trace};

use super::process::run_shell;

/// A lazy command whose consumers share one execution and completion result.
#[derive(Trace, Finalize, JsData)]
struct ShellCommand {
    state: CommandState,
}

#[derive(Trace, Finalize)]
enum CommandState {
    // Preserve parsing errors until the command is consumed.
    Pending(JsResult<Vec<String>>),
    // The promise retains both running and settled execution state.
    Terminal(JsPromise),
}

impl Class for ShellCommand {
    const NAME: &'static str = "ShellCommand";

    fn data_constructor(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<Self> {
        Err(JsNativeError::typ()
            .with_message("use a $ template to create a ShellCommand")
            .into())
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        class.method(
            js_string!("then"),
            2,
            NativeFunction::from_fn_ptr(command_then),
        );
        class.method(
            js_string!("catch"),
            1,
            NativeFunction::from_fn_ptr(command_catch),
        );
        class.method(
            js_string!("finally"),
            1,
            NativeFunction::from_fn_ptr(command_finally),
        );
        Ok(())
    }
}

fn completion(this: &JsValue, context: &mut Context) -> JsResult<JsPromise> {
    let invalid_receiver = || -> JsError {
        JsNativeError::typ()
            .with_message("invalid ShellCommand receiver")
            .into()
    };
    let object = this.as_object().ok_or_else(invalid_receiver)?;
    let mut command = object
        .downcast_mut::<ShellCommand>()
        .ok_or_else(invalid_receiver)?;
    match &command.state {
        CommandState::Terminal(promise) => return Ok(promise.clone()),
        CommandState::Pending(_) => {}
    }
    // Creating the promise only queues work; it cannot invoke JS or poll the child here.
    let CommandState::Pending(input) = &command.state else {
        unreachable!()
    };
    let promise = match input.clone() {
        Ok(words) => JsPromise::from_async_fn(async move |_| run_shell(words).await, context),
        Err(error) => JsPromise::reject(error, context)?,
    };
    command.state = CommandState::Terminal(promise.clone());
    Ok(promise)
}

fn command_then(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let promise = completion(this, context)?;
    Ok(promise
        .then(
            args.get_or_undefined(0)
                .as_callable()
                .and_then(JsFunction::from_object),
            args.get_or_undefined(1)
                .as_callable()
                .and_then(JsFunction::from_object),
            context,
        )?
        .into())
}

fn command_catch(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let promise = completion(this, context)?;
    Ok(promise
        .then(
            None,
            args.get_or_undefined(0)
                .as_callable()
                .and_then(JsFunction::from_object),
            context,
        )?
        .into())
}

fn command_finally(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let promise = completion(this, context)?;
    let chained = match args
        .get_or_undefined(0)
        .as_callable()
        .and_then(JsFunction::from_object)
    {
        Some(callback) => promise.finally(callback, context)?,
        None => promise.then(None, None, context)?,
    };
    Ok(chained.into())
}

pub(super) fn create(input: JsResult<Vec<String>>, context: &mut Context) -> JsResult<JsValue> {
    if context.realm().get_class::<ShellCommand>().is_none() {
        let mut builder = ClassBuilder::new::<ShellCommand>(context);
        ShellCommand::init(&mut builder)?;
        let class = builder.build();
        context.realm().register_class::<ShellCommand>(class);
    }
    let state = CommandState::Pending(input);
    Ok(ShellCommand::from_data(ShellCommand { state }, context)?.into())
}
