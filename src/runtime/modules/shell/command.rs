use std::collections::BTreeMap;

use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
    object::builtins::{JsFunction, JsPromise},
    property::PropertyKey,
};
use boa_gc::{Finalize, Trace};

use super::{
    ShellEnvironment, lines, output,
    process::{OutputMode, run_shell},
};

/// A lazy command whose consumers share one execution and completion result.
#[derive(Trace, Finalize, JsData)]
struct ShellCommand {
    state: CommandState,
    #[unsafe_ignore_trace]
    environment: ShellEnvironment,
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
        add_method!("nothrow" command_nothrow/0);
        add_method!("json" command_json/0);
        add_method!("blob" command_blob/0);
        add_method!("lines" command_lines/0);
        add_method!("env" command_env/1);

        Ok(())
    }
}

fn command_env(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let invalid_receiver = || JsNativeError::typ().with_message("invalid ShellCommand receiver");
    let consumed = || JsNativeError::typ().with_message("env() requires an unconsumed command");
    let object = this.as_object().ok_or_else(invalid_receiver)?;
    {
        let command = object
            .downcast_ref::<ShellCommand>()
            .ok_or_else(invalid_receiver)?;
        if !matches!(command.state, CommandState::Pending(_)) {
            return Err(consumed().into());
        }
    }

    let argument = args.get_or_undefined(0);
    let values = argument.as_object().ok_or_else(|| {
        JsNativeError::typ().with_message("env() requires an object of string keys and values")
    })?;
    // Property access can execute JavaScript. Hold no command borrow until the
    // complete snapshot is validated, then check again in case it was consumed.
    let mut variables = BTreeMap::new();
    for key in values.own_property_keys(context)? {
        let name = match &key {
            PropertyKey::String(name) => name.to_std_string().map_err(|_| {
                JsNativeError::typ().with_message("env() keys must contain valid Unicode")
            })?,
            PropertyKey::Index(index) => index.get().to_string(),
            PropertyKey::Symbol(_) => {
                return Err(JsNativeError::typ()
                    .with_message("env() keys must be strings")
                    .into());
            }
        };
        if name.is_empty() || name.contains(['=', '\0']) {
            return Err(JsNativeError::typ()
                .with_message("env() keys must be nonempty and cannot contain '=' or NUL")
                .into());
        }
        let value = values.get(key, context)?;
        let value = value
            .as_string()
            .ok_or_else(|| JsNativeError::typ().with_message("env() values must be strings"))?
            .to_std_string()
            .map_err(|_| {
                JsNativeError::typ().with_message("env() values must contain valid Unicode")
            })?;
        if value.contains('\0') {
            return Err(JsNativeError::typ()
                .with_message("env() values cannot contain NUL")
                .into());
        }
        variables.insert(name, value);
    }

    let mut command = object
        .downcast_mut::<ShellCommand>()
        .ok_or_else(invalid_receiver)?;
    if !matches!(command.state, CommandState::Pending(_)) {
        return Err(consumed().into());
    }
    command.environment.variables.extend(variables);
    Ok(this.clone())
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
        if requested != OutputMode::Terminal {
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
        if (requested == OutputMode::Capture && *mode == OutputMode::Terminal)
            || (requested == OutputMode::FullCapture && *mode != OutputMode::FullCapture)
        {
            return Err(JsNativeError::typ()
                .with_message("cannot capture requested output after another execution mode has been selected")
                .into());
        }
        return Ok(promise.clone());
    }

    // Deal with the pending state -- build the promise to run it.
    let CommandState::Pending(input) = &command.state else {
        unreachable!()
    };
    let environment = command.environment.clone();
    let promise = match input.clone() {
        Ok(words) => JsPromise::from_async_fn(
            async move |_| {
                let bytes = run_shell(words, requested, environment).await?;
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
    let check = NativeFunction::from_fn_ptr(|this, args, context| {
        if !args.get_or_undefined(0).is_undefined() {
            output::checked(this, args, context)?;
        }
        Ok(JsValue::undefined())
    })
    .to_js_function(context.realm());
    promise.then(Some(check), None, context)
}

/// Convert shared stdout bytes after successful execution, without rerunning
/// the command.
fn read(this: &JsValue, convert: NativeFunction, context: &mut Context) -> JsResult<JsValue> {
    let promise = match execution(this, OutputMode::Capture, context) {
        Ok(promise) => promise,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    let check = NativeFunction::from_fn_ptr(output::checked).to_js_function(context.realm());
    let promise = promise.then(Some(check), None, context)?;
    let convert = convert.to_js_function(context.realm());
    Ok(promise.then(Some(convert), None, context)?.into())
}

fn command_nothrow(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let promise = match execution(this, OutputMode::FullCapture, context) {
        Ok(promise) => promise,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    let convert = NativeFunction::from_fn_ptr(output::nothrow).to_js_function(context.realm());
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
    let iterator = lines::create(input.clone(), command.environment.clone(), context)?;
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
pub(super) fn create(
    input: JsResult<Vec<String>>,
    environment: ShellEnvironment,
    context: &mut Context,
) -> JsResult<JsValue> {
    // Register the class once per realm so commands share its prototype and methods.
    if context.realm().get_class::<ShellCommand>().is_none() {
        let mut builder = ClassBuilder::new::<ShellCommand>(context);
        ShellCommand::init(&mut builder)?;
        let class = builder.build();
        context.realm().register_class::<ShellCommand>(class);
    }
    // Keep the input until the first await or read.
    let state = CommandState::Pending(input);
    Ok(ShellCommand::from_data(ShellCommand { state, environment }, context)?.into())
}
