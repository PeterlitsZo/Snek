use std::{cell::RefCell, rc::Rc};

use boa_engine::{
    Context, JsNativeError, JsResult, Module, NativeFunction, js_string,
    module::{ModuleLoader, ModuleRequest, Referrer, SyntheticModuleInitializer},
    object::FunctionObjectBuilder,
};

#[derive(Default)]
pub(super) struct SnekModuleLoader {
    module: RefCell<Option<Module>>,
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

        // Make the snek module.
        let module = Module::synthetic(
            &[
                js_string!("$"),
                js_string!("S3Client"),
                js_string!("Buffer"),
                js_string!("Blob"),
            ],
            SyntheticModuleInitializer::from_copy_closure(|module, context| {
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

                // Export the functions and others.
                module.set_export(&js_string!("$"), function.into())?;
                module.set_export(&js_string!("Buffer"), buffer)?;
                module.set_export(&js_string!("Blob"), blob)?;
                module.set_export(&js_string!("S3Client"), s3)
            }),
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
