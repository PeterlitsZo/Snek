use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
    object::builtins::{JsFunction, JsPromise},
};
use boa_gc::{Finalize, Trace};

use super::{
    lines, output,
    process::{OutputMode, run_shell},
};

/// A lazy command whose consumers share one execution and completion result.
#[derive(Trace, Finalize, JsData)]
struct ShellCommand {
    state: CommandState,
}

#[derive(Trace, Finalize)]
enum CommandState {
    // Preserve parsing errors until the command is consumed.
    Pending(JsResult<Vec<String>>),
    // Keep the sole iterator and its process state alive for completion waits.
    Streaming(JsObject),
    // The promise retains both running and settled execution state.
    Running {
        #[unsafe_ignore_trace]
        mode: OutputMode,
        promise: JsPromise,
    },
}

impl Class for ShellCommand {
    const NAME: &'static str = "ShellCommand";

    fn data_constructor(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<Self> {
        Err(JsNativeError::typ()
            .with_message("use a $ template to create a ShellCommand")
            .into())
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        macro_rules! add_method {
            ($name:literal $fn:ident/$len:literal) => {
                class.method(js_string!($name), $len, NativeFunction::from_fn_ptr($fn));
            };
        }

        // Add the methods.
        add_method!("then" command_then/2);
        add_method!("catch" command_catch/1);
        add_method!("finally" command_finally/1);
        add_method!("text" command_text/0);
        add_method!("json" command_json/0);
        add_method!("blob" command_blob/0);
        add_method!("lines" command_lines/0);

        Ok(())
    }
}

/// Start once in the `requested` mode, then reuse the same result or error.
fn execution(this: &JsValue, requested: OutputMode, context: &mut Context) -> JsResult<JsPromise> {
    let invalid_receiver = || -> JsError {
        JsNativeError::typ()
            .with_message("invalid ShellCommand receiver")
            .into()
    };

    // Convert `this` into a `ShellCommand` instance.
    let object = this.as_object().ok_or_else(invalid_receiver)?;
    let mut command = object
        .downcast_mut::<ShellCommand>()
        .ok_or_else(invalid_receiver)?;

    if let CommandState::Streaming(iterator) = &command.state {
        if requested == OutputMode::Capture {
            return Err(JsNativeError::typ()
                .with_message("cannot capture output after lines() has selected streaming mode")
                .into());
        }
        let iterator = iterator.clone();
        drop(command);
        return lines::completion(&iterator, context);
    }

    // Already running? Just return.
    if let CommandState::Running { mode, promise } = &command.state {
        // Too late to capture the output.
        if requested == OutputMode::Capture && *mode != OutputMode::Capture {
            return Err(JsNativeError::typ()
                .with_message("cannot capture output after terminal execution has started")
                .into());
        }
        return Ok(promise.clone());
    }

    // Deal with the pending state -- build the promise to run it.
    let CommandState::Pending(input) = &command.state else {
        unreachable!()
    };
    let promise = match input.clone() {
        Ok(words) => JsPromise::from_async_fn(
            async move |_| {
                let bytes = run_shell(words, requested).await?;
                Ok(output::captured(bytes))
            },
            context,
        ),
        // Surface saved template errors only when the command is consumed.
        Err(error) => JsPromise::reject(error, context)?,
    };

    // Change the state.
    command.state = CommandState::Running {
        mode: requested,
        promise: promise.clone(),
    };

    // Return the promise.
    Ok(promise)
}

/// Wait for execution, resolving to undefined and preserving any process error.
fn completion(this: &JsValue, context: &mut Context) -> JsResult<JsPromise> {
    let promise = execution(this, OutputMode::Terminal, context)?;
    let discard = NativeFunction::from_fn_ptr(|_, _, _| Ok(JsValue::undefined()))
        .to_js_function(context.realm());
    promise.then(Some(discard), None, context)
}

/// Convert shared stdout bytes after successful execution, without rerunning
/// the command.
fn read(this: &JsValue, convert: NativeFunction, context: &mut Context) -> JsResult<JsValue> {
    let promise = match execution(this, OutputMode::Capture, context) {
        Ok(promise) => promise,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    let convert = convert.to_js_function(context.realm());
    Ok(promise.then(Some(convert), None, context)?.into())
}

fn command_text(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    read(this, NativeFunction::from_fn_ptr(output::text), context)
}

fn command_json(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    read(this, NativeFunction::from_fn_ptr(output::json), context)
}

fn command_blob(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    read(this, NativeFunction::from_fn_ptr(output::blob), context)
}

/// Reserve streaming mode once; the iterator starts the process on first consumption.
fn command_lines(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let object = this
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid ShellCommand receiver"))?;
    let mut command = object
        .downcast_mut::<ShellCommand>()
        .ok_or_else(|| JsNativeError::typ().with_message("invalid ShellCommand receiver"))?;
    let CommandState::Pending(input) = &command.state else {
        return Err(JsNativeError::typ()
            .with_message("lines() requires an unconsumed command and allows only one iterator")
            .into());
    };
    let iterator = lines::create(input.clone(), context)?;
    command.state = CommandState::Streaming(iterator.clone());
    Ok(iterator.into())
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

/// Wrap parsed arguments or a parsing error in a lazy command without starting a process.
pub(super) fn create(input: JsResult<Vec<String>>, context: &mut Context) -> JsResult<JsValue> {
    // Register the class once per realm so commands share its prototype and methods.
    if context.realm().get_class::<ShellCommand>().is_none() {
        let mut builder = ClassBuilder::new::<ShellCommand>(context);
        ShellCommand::init(&mut builder)?;
        let class = builder.build();
        context.realm().register_class::<ShellCommand>(class);
    }
    // Keep the input until the first await or read.
    let state = CommandState::Pending(input);
    Ok(ShellCommand::from_data(ShellCommand { state }, context)?.into())
}
