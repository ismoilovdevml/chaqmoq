//! # Chaqmoq runtime
//!
//! Embeds V8 (through `deno_core`) and layers a small, secure-by-default
//! platform on top of it: web-standard globals, a `Chaqmoq.*` namespace for
//! system access, an ES module graph that understands TypeScript, and a
//! permission check in front of every syscall.
//!
//! ```no_run
//! # async fn example() -> anyhow::Result<()> {
//! use chaqmoq_runtime::{ChaqmoqWorker, WorkerOptions};
//! use chaqmoq_runtime::permissions::Permissions;
//!
//! let main = deno_core::resolve_path("main.ts", &std::env::current_dir()?)?;
//! let mut worker = ChaqmoqWorker::new(WorkerOptions {
//!     main_module: main.clone(),
//!     permissions: Permissions::allow_all(),
//!     ..Default::default()
//! })?;
//! worker.run_main_module().await?;
//! # Ok(())
//! # }
//! ```

pub mod colors;
pub mod errors;
pub mod module_loader;
pub mod ops;
pub mod permissions;

use std::rc::Rc;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::error::CoreError;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::PollEventLoopOptions;
use deno_core::RuntimeOptions;

use crate::module_loader::ChaqmoqModuleLoader;
use crate::ops::http::HttpState;
use crate::permissions::Permissions;

/// The runtime version, taken from the crate manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The V8 version Chaqmoq was linked against.
pub fn v8_version() -> &'static str {
    deno_core::v8::VERSION_STRING
}

/// Where downloaded remote modules are cached, if the platform has a cache
/// directory at all.
pub fn cache_dir() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("chaqmoq"))
}

/// Per-program values that ops read out of `OpState`.
pub struct RuntimeState {
    /// Script arguments — everything after the module path on the command line.
    pub args: Vec<String>,
    pub main_module: String,
    /// Monotonic clock start, backing `performance.now()`.
    pub start: Instant,
    /// Wall-clock milliseconds at start, backing `performance.timeOrigin`.
    pub time_origin: f64,
}

impl RuntimeState {
    fn new(args: Vec<String>, main_module: String) -> Self {
        let time_origin = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        Self {
            args,
            main_module,
            start: Instant::now(),
            time_origin,
        }
    }
}

deno_core::extension!(
    chaqmoq,
    ops = [
        // filesystem
        ops::fs::op_chq_read_file,
        ops::fs::op_chq_read_file_sync,
        ops::fs::op_chq_read_text_file,
        ops::fs::op_chq_read_text_file_sync,
        ops::fs::op_chq_write_file,
        ops::fs::op_chq_write_file_sync,
        ops::fs::op_chq_read_dir,
        ops::fs::op_chq_read_dir_sync,
        ops::fs::op_chq_mkdir,
        ops::fs::op_chq_mkdir_sync,
        ops::fs::op_chq_remove,
        ops::fs::op_chq_remove_sync,
        ops::fs::op_chq_stat,
        ops::fs::op_chq_stat_sync,
        ops::fs::op_chq_exists_sync,
        ops::fs::op_chq_copy_file,
        ops::fs::op_chq_rename,
        ops::fs::op_chq_real_path_sync,
        ops::fs::op_chq_make_temp_dir,
        // process & platform
        ops::os::op_chq_build_info,
        ops::os::op_chq_args,
        ops::os::op_chq_main_module,
        ops::os::op_chq_pid,
        ops::os::op_chq_cwd,
        ops::os::op_chq_chdir,
        ops::os::op_chq_exit,
        ops::os::op_chq_env_get,
        ops::os::op_chq_env_set,
        ops::os::op_chq_env_delete,
        ops::os::op_chq_env_to_object,
        ops::os::op_chq_hostname,
        ops::os::op_chq_uptime,
        ops::os::op_chq_now,
        ops::os::op_chq_time_origin,
        // crypto
        ops::crypto::op_chq_get_random_values,
        ops::crypto::op_chq_random_uuid,
        ops::crypto::op_chq_digest,
        // encoding
        ops::encoding::op_chq_btoa,
        ops::encoding::op_chq_atob,
        // url
        ops::url::op_chq_url_parse,
        ops::url::op_chq_url_set,
        ops::url::op_chq_url_parse_search_params,
        ops::url::op_chq_url_stringify_search_params,
        // http
        ops::http::op_chq_fetch,
        ops::http::op_chq_fetch_take_body,
        ops::http::op_chq_serve_listen,
        ops::http::op_chq_serve_next,
        ops::http::op_chq_serve_read_body,
        ops::http::op_chq_serve_respond,
        ops::http::op_chq_serve_close,
        ops::http::op_chq_serve_addr,
    ],
    esm_entry_point = "ext:chaqmoq/99_bootstrap.js",
    esm = [
        dir "js",
        "00_errors.js",
        "01_web.js",
        "02_console.js",
        "03_timers.js",
        "04_fs.js",
        "05_http.js",
        "06_crypto.js",
        "07_process.js",
        "08_testing.js",
        "99_bootstrap.js",
    ],
    options = {
        permissions: Permissions,
        runtime_state: RuntimeState,
    },
    state = |state, options| {
        state.put(options.permissions);
        state.put(options.runtime_state);
        state.put(HttpState::default());
    },
);

/// How to construct a [`ChaqmoqWorker`].
pub struct WorkerOptions {
    /// Module evaluated by [`ChaqmoqWorker::run_main_module`].
    pub main_module: ModuleSpecifier,
    /// Values exposed as `Chaqmoq.args`.
    pub args: Vec<String>,
    pub permissions: Permissions,
    /// Bypass the on-disk cache for remote modules.
    pub reload: bool,
}

impl Default for WorkerOptions {
    fn default() -> Self {
        Self {
            main_module: ModuleSpecifier::parse("file:///main.js")
                .expect("the default specifier is valid"),
            args: Vec::new(),
            permissions: Permissions::none(),
            reload: false,
        }
    }
}

/// One isolate running one program.
pub struct ChaqmoqWorker {
    js_runtime: JsRuntime,
    main_module: ModuleSpecifier,
}

impl ChaqmoqWorker {
    pub fn new(options: WorkerOptions) -> Result<Self, anyhow::Error> {
        let WorkerOptions {
            main_module,
            args,
            permissions,
            reload,
        } = options;

        let module_loader = Rc::new(ChaqmoqModuleLoader::new(permissions.clone(), reload));
        let runtime_state = RuntimeState::new(args, main_module.to_string());

        let js_runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(module_loader),
            extensions: vec![chaqmoq::init(permissions, runtime_state)],
            ..Default::default()
        });

        Ok(Self {
            js_runtime,
            main_module,
        })
    }

    /// Loads, evaluates and then drives the event loop until it is empty.
    pub async fn run_main_module(&mut self) -> Result<(), CoreError> {
        let specifier = self.main_module.clone();
        let module_id = self.js_runtime.load_main_es_module(&specifier).await?;
        self.evaluate_and_run(module_id).await
    }

    /// Same as [`Self::run_main_module`], but with the source supplied inline —
    /// used by `chaqmoq eval` and the REPL.
    pub async fn run_main_module_from_code(&mut self, code: String) -> Result<(), CoreError> {
        let specifier = self.main_module.clone();
        let module_id = self
            .js_runtime
            .load_main_es_module_from_code(&specifier, code)
            .await?;
        self.evaluate_and_run(module_id).await
    }

    async fn evaluate_and_run(&mut self, module_id: deno_core::ModuleId) -> Result<(), CoreError> {
        let mut evaluate = std::pin::pin!(self.js_runtime.mod_evaluate(module_id));

        // Poll `evaluate` first. A module that throws at the top level rejects
        // its evaluation promise synchronously, and checking it before the
        // event loop means the failure is reported as the uncaught exception it
        // is -- not as an unhandled promise rejection.
        if let std::task::Poll::Ready(result) = futures::poll!(std::pin::Pin::new(&mut evaluate)) {
            result?;
            // Evaluation finished, but timers and pending I/O still have to run.
            return self
                .js_runtime
                .run_event_loop(PollEventLoopOptions::default())
                .await;
        }

        // Top-level await: the module is still running, so drive both.
        self.js_runtime
            .run_event_loop(PollEventLoopOptions::default())
            .await?;
        evaluate.await
    }

    /// Runs a classic (non-module) script in the main context.
    ///
    /// `code` is taken as an owned `String` because the trait `deno_core` uses
    /// for script sources is not part of its public API.
    pub fn execute_script(
        &mut self,
        name: &'static str,
        code: impl Into<String>,
    ) -> Result<deno_core::v8::Global<deno_core::v8::Value>, Box<deno_core::error::JsError>> {
        self.js_runtime.execute_script(name, code.into())
    }

    pub async fn run_event_loop(&mut self) -> Result<(), CoreError> {
        self.js_runtime
            .run_event_loop(PollEventLoopOptions::default())
            .await
    }

    /// Escape hatch for callers that need the raw isolate (the REPL formats
    /// completion values through it).
    pub fn js_runtime(&mut self) -> &mut JsRuntime {
        &mut self.js_runtime
    }
}
