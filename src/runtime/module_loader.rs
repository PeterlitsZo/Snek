use std::{cell::RefCell, path::Path, rc::Rc};

use boa_engine::{
    Context, JsNativeError, JsResult, JsValue, Module, NativeFunction, js_string,
    module::{ModuleLoader, ModuleRequest, Referrer, SyntheticModuleInitializer},
    object::{FunctionObjectBuilder, builtins::JsArray},
};

pub(super) struct SnekModuleLoader {
    module: RefCell<Option<Module>>,
    argv: Vec<String>,
    script: String,
}

impl SnekModuleLoader {
    pub(super) fn new(path: &Path, argv: Vec<String>) -> Self {
        Self {
            module: RefCell::new(None),
            argv,
            script: path.to_string_lossy().into_owned(),
        }
    }
}

impl ModuleLoader for SnekModuleLoader {
    async fn load_imported_module(
        self: Rc<Self>,
        _referrer: Referrer,
        request: ModuleRequest,
        context: &RefCell<&mut Context>,
    ) -> JsResult<Module> {
        // Make sure we only import from snek.
        if request.specifier() != &js_string!("snek") {
            return Err(JsNativeError::typ()
                .with_message("only static imports from \"snek\" are supported")
                .into());
        }

        // If it is imported, just return.
        if let Some(module) = self.module.borrow().as_ref() {
            return Ok(module.clone());
        }

        let argv = JsArray::from_iter(
            self.argv
                .iter()
                .map(|value| JsValue::from(js_string!(value.as_str()))),
            &mut context.borrow_mut(),
        );
        let captures = (JsValue::from(argv), js_string!(self.script.as_str()));

        // Make the snek module.
        let module = Module::synthetic(
            &[
                js_string!("argv"),
                js_string!("Cli"),
                js_string!("$"),
                js_string!("S3Client"),
                js_string!("Buffer"),
                js_string!("Blob"),
            ],
            SyntheticModuleInitializer::from_copy_closure_with_captures(
                |module, captures, context| {
                    // Build the `$` function.
                    let function = FunctionObjectBuilder::new(
                        context.realm(),
                        NativeFunction::from_fn_ptr(super::modules::shell::dollar),
                    )
                    .name(js_string!("$"))
                    .length(1)
                    .build();

                    // Build the `Buffer`.
                    let buffer = super::modules::buffer::constructor(context)?;

                    // Build the `Blob`.
                    let blob = super::modules::blob::constructor(context)?;

                    // Build the `S3Client`.
                    let s3 = super::modules::s3::constructor(context)?;

                    // Build the `Cli`.
                    let cli = super::modules::cli::constructor(
                        captures.0.clone(),
                        captures.1.clone(),
                        context,
                    )?;

                    // Export the functions and others.
                    module.set_export(&js_string!("argv"), captures.0.clone())?;
                    module.set_export(&js_string!("Cli"), cli)?;
                    module.set_export(&js_string!("$"), function.into())?;
                    module.set_export(&js_string!("Buffer"), buffer)?;
                    module.set_export(&js_string!("Blob"), blob)?;
                    module.set_export(&js_string!("S3Client"), s3)
                },
                captures,
            ),
            None,
            None,
            &mut context.borrow_mut(),
        );

        // Remember it.
        *self.module.borrow_mut() = Some(module.clone());

        // Return the module.
        Ok(module)
    }
}
