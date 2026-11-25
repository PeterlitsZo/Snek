use std::{cell::RefCell, path::Path, rc::Rc};

use anyhow::{Result, anyhow, bail};
use boa_engine::{Context, Module, Source, builtins::promise::PromiseState, job::JobExecutor};
use boa_runtime::extensions::{ConsoleExtension, FetchExtension};

mod dns;
mod fetch;
mod jobs;
mod module_loader;
mod modules;
mod state;

use jobs::SnekJobExecutor;
use module_loader::SnekModuleLoader;

// Execute the JavaScript code.
pub(crate) fn execute(
    path: &Path,
    script: &Path,
    javascript: &str,
    argv: Vec<String>,
) -> Result<u8> {
    // Build the tokio runtime.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| anyhow!("Tokio initialization failed: {error}"))?;

    // Build the job excutor.
    let jobs = Rc::new(SnekJobExecutor::default());

    // Build the context to execute the JavaScript code.
    let mut context = Context::builder()
        .module_loader(Rc::new(SnekModuleLoader::new(path, script, argv)?))
        .job_executor(jobs.clone())
        .build()
        .map_err(|error| anyhow!("runtime initialization failed: {error}"))?;

    let state = state::RuntimeState::new()?;
    context.insert_data(modules::shell::ShellEnvironment::new(state.clone()));
    context.insert_data(state.clone());

    boa_runtime::register(
        (
            ConsoleExtension::default(),
            FetchExtension(fetch::HttpFetcher::new()?),
        ),
        None,
        &mut context,
    )
    .map_err(|error| anyhow!("runtime extensions initialization failed: {error}"))?;

    // Build the module.
    let module = Module::parse(
        Source::from_bytes(javascript).with_path(path),
        None,
        &mut context,
    )
    .map_err(|error| anyhow!("{error}"))?;

    // Loads, links and evaluates this module.
    let completion = module.load_link_evaluate(&mut context);
    runtime
        .block_on(jobs.run_jobs_async(&RefCell::new(&mut context)))
        .map_err(|error| anyhow!("{error}"))?;

    // Check the state.
    match completion.state() {
        PromiseState::Fulfilled(_) => {
            modules::cli::check_actions(&context).map_err(|error| anyhow!("{error}"))?;
            Ok(state
                .0
                .lock()
                .map_err(|_| anyhow!("process state lock poisoned"))?
                .exit_code)
        }
        PromiseState::Rejected(error) => {
            let error = error
                .to_string(&mut context)
                .map(|text| text.to_std_string_escaped())
                .unwrap_or_else(|_| "module evaluation rejected with an unprintable value".into());
            bail!("{error}")
        }
        PromiseState::Pending => bail!("top-level await did not complete: no pending tasks remain"),
    }
}
