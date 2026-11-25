use std::{cell::Cell, rc::Rc};

use boa_engine::{
    Context, JsData, JsError, JsNativeError, JsObject, JsResult, JsSymbol, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
    object::{ObjectInitializer, builtins::JsPromise},
    property::Attribute,
};
use boa_gc::{Finalize, Trace};

use super::{ShellEnvironment, line_process::LineProcess, shell_error};

/// One iterator owns the stream; command completion observes the same process.
#[derive(Clone, Trace, Finalize, JsData)]
struct LineIterator {
    // These fields contain only Rust state; parsing errors below are traced normally.
    #[unsafe_ignore_trace]
    process: Option<Rc<LineProcess>>,
    #[unsafe_ignore_trace]
    error_consumed: Rc<Cell<bool>>,
    error: Option<JsError>,
}

fn receiver(this: &JsValue) -> JsResult<LineIterator> {
    let object = this.as_object().ok_or_else(invalid_receiver)?;
    let iterator = object
        .downcast_ref::<LineIterator>()
        .ok_or_else(invalid_receiver)?;
    Ok(iterator.clone())
}

fn invalid_receiver() -> JsError {
    JsNativeError::typ()
        .with_message("invalid shell lines receiver")
        .into()
}

impl Class for LineIterator {
    const NAME: &'static str = "ShellLines";

    fn data_constructor(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<Self> {
        Err(JsNativeError::typ()
            .with_message("use ShellCommand.lines() to create a line iterator")
            .into())
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        class.method(js_string!("next"), 0, NativeFunction::from_fn_ptr(next));
        class.method(js_string!("return"), 0, NativeFunction::from_fn_ptr(close));
        class.method(
            JsSymbol::async_iterator(),
            0,
            NativeFunction::from_fn_ptr(|this, _, _| {
                receiver(this)?;
                Ok(this.clone())
            }),
        );
        Ok(())
    }
}

pub(super) fn create(
    input: JsResult<Vec<String>>,
    environment: ShellEnvironment,
    context: &mut Context,
) -> JsResult<JsObject> {
    if context.realm().get_class::<LineIterator>().is_none() {
        let mut builder = ClassBuilder::new::<LineIterator>(context);
        LineIterator::init(&mut builder)?;
        let class = builder.build();
        context.realm().register_class::<LineIterator>(class);
    }
    let (process, error) = match input {
        Ok(words) => (Some(Rc::new(LineProcess::new(words, environment))), None),
        Err(error) => (None, Some(error)),
    };
    LineIterator::from_data(
        LineIterator {
            process,
            error,
            error_consumed: Rc::new(Cell::new(false)),
        },
        context,
    )
}

fn iteration(line: Option<Vec<u8>>, context: &mut Context) -> JsValue {
    let done = line.is_none();
    let value = line
        .map(|bytes| JsValue::from(js_string!(String::from_utf8_lossy(&bytes).as_ref())))
        .unwrap_or_else(JsValue::undefined);
    ObjectInitializer::new(context)
        .property(js_string!("value"), value, Attribute::all())
        .property(js_string!("done"), done, Attribute::all())
        .build()
        .into()
}

/// Each next() waits for one complete line; UTF-8 decoding happens after framing.
fn next(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let iterator = match receiver(this) {
        Ok(iterator) => iterator,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    if let Some(error) = iterator.error.as_ref() {
        if !iterator.error_consumed.replace(true) {
            return Ok(JsPromise::reject(error.clone(), context)?.into());
        }
        let done = iteration(None, context);
        return Ok(JsPromise::resolve(done, context)?.into());
    }
    let process = iterator.process.clone().expect("valid input has a process");
    Ok(JsPromise::from_async_fn(
        async move |context| {
            let line = process.next().await.map_err(shell_error)?;
            Ok(iteration(line, &mut context.borrow_mut()))
        },
        context,
    )
    .into())
}

/// Cancel immediately, then resolve only after the direct child has been reaped.
fn close(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let iterator = match receiver(this) {
        Ok(iterator) => iterator,
        Err(error) => return Ok(JsPromise::reject(error, context)?.into()),
    };
    iterator.error_consumed.set(true);
    if let Some(process) = iterator.process.as_ref() {
        process.cancel();
    }
    let process = iterator.process.clone();
    Ok(JsPromise::from_async_fn(
        async move |context| {
            if let Some(process) = process {
                process.close().await.map_err(shell_error)?;
            }
            Ok(iteration(None, &mut context.borrow_mut()))
        },
        context,
    )
    .into())
}

/// Waiting for completion does not acquire the iterator's read lock.
pub(super) fn completion(iterator: &JsObject, context: &mut Context) -> JsResult<JsPromise> {
    let iterator = receiver(&iterator.clone().into())?;
    if let Some(error) = iterator.error.as_ref() {
        return JsPromise::reject(error.clone(), context);
    }
    let process = iterator.process.clone().expect("valid input has a process");
    Ok(JsPromise::from_async_fn(
        async move |_| {
            process.completion().await.map_err(shell_error)?;
            Ok(JsValue::undefined())
        },
        context,
    ))
}
