use std::{cell::RefCell, path::Path, rc::Rc};

use anyhow::{Context as _, Result, anyhow};
use boa_engine::{
    Context, JsNativeError, JsObject, JsResult, JsValue, Module, NativeFunction, js_string,
    module::{ModuleLoader, ModuleRequest, Referrer, SyntheticModuleInitializer},
    object::{FunctionObjectBuilder, builtins::JsArray},
};

pub(super) struct SnekModuleLoader {
    module: RefCell<Option<Module>>,
    argv: Vec<String>,
    script: String,
    meta: [String; 4],
}

impl SnekModuleLoader {
    pub(super) fn new(path: &Path, script: &Path, argv: Vec<String>) -> Result<Self> {
        let absolute = script.to_str().context("script path is not valid UTF-8")?;
        let dir = script
            .parent()
            .and_then(Path::to_str)
            .context("script directory is not valid UTF-8")?;
        let file = script
            .file_name()
            .and_then(|name| name.to_str())
            .context("script filename is not valid UTF-8")?;
        let url = url::Url::from_file_path(script)
            .map_err(|_| anyhow!("cannot convert script path to a file URL"))?;
        Ok(Self {
            module: RefCell::new(None),
            argv,
            script: path.to_string_lossy().into_owned(),
            meta: [dir.into(), file.into(), absolute.into(), url.into()],
        })
    }
}

impl ModuleLoader for SnekModuleLoader {
    fn init_import_meta(
        self: Rc<Self>,
        import_meta: &JsObject,
        _module: &Module,
        context: &mut Context,
    ) {
        for (name, value) in ["dir", "file", "path", "url"].into_iter().zip(&self.meta) {
            // Boa supplies a fresh, extensible ordinary object.
            import_meta
                .create_data_property_or_throw(
                    js_string!(name),
                    js_string!(value.as_str()),
                    context,
                )
                .expect("import.meta must accept data properties");
        }
    }

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
                js_string!("file"),
                js_string!("argv"),
                js_string!("process"),
                js_string!("Cli"),
                js_string!("$"),
                js_string!("S3Client"),
                js_string!("Buffer"),
                js_string!("Blob"),
                js_string!("Semver"),
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

                    let semver = super::modules::semver::constructor(context)?;

                    // Build the `S3Client`.
                    let s3 = super::modules::s3::constructor(context)?;

                    // Build the `Cli`.
                    let cli = super::modules::cli::constructor(
                        captures.0.clone(),
                        captures.1.clone(),
                        context,
                    )?;

                    let file = FunctionObjectBuilder::new(
                        context.realm(),
                        NativeFunction::from_fn_ptr(super::modules::file::file),
                    )
                    .name(js_string!("file"))
                    .length(1)
                    .build();
                    module.set_export(&js_string!("file"), file.into())?;

                    let process = super::modules::process::create(captures.0.clone(), context)?;
                    module.set_export(&js_string!("process"), process)?;

                    // Export the functions and others.
                    module.set_export(&js_string!("argv"), captures.0.clone())?;
                    module.set_export(&js_string!("Cli"), cli)?;
                    module.set_export(&js_string!("$"), function.into())?;
                    module.set_export(&js_string!("Buffer"), buffer)?;
                    module.set_export(&js_string!("Blob"), blob)?;
                    module.set_export(&js_string!("Semver"), semver)?;
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
