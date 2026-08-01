//! `chaqmoq` — the command line entry point.

mod flags;
mod repl;
mod testing;
mod watcher;

use std::io::IsTerminal;

use chaqmoq_runtime::colors;
use chaqmoq_runtime::errors::format_core_error;
use chaqmoq_runtime::permissions::Permissions;
use chaqmoq_runtime::ChaqmoqWorker;
use chaqmoq_runtime::WorkerOptions;
use clap::Parser;
use deno_core::ModuleSpecifier;

use crate::flags::Cli;
use crate::flags::Command;
use crate::flags::PermissionFlags;

fn main() {
    let cli = Cli::parse();

    let exit_code = match run(cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{}: {error}", colors::red_bold("error"));
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> Result<i32, anyhow::Error> {
    // A current-thread runtime is required: V8 isolates are not `Send`, and
    // every op runs on the isolate's thread.
    let tokio = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    match cli.command {
        Command::Run {
            permissions,
            watch,
            script,
            script_args,
        } => {
            if watch {
                watcher::run_with_watch(&tokio, &permissions, &script, &script_args)
            } else {
                tokio.block_on(run_script(&permissions, &script, &script_args))
            }
        }

        Command::Eval {
            permissions,
            ts,
            code,
        } => tokio.block_on(run_eval(&permissions, &code, ts)),

        Command::Repl { permissions } => tokio.block_on(repl::start(&permissions)),

        Command::Test {
            permissions,
            filter,
            paths,
        } => tokio.block_on(testing::run_tests(&permissions, &paths, filter.as_deref())),

        Command::Info => {
            print_info();
            Ok(0)
        }
    }
}

/// Builds a worker with the colour setting already applied.
pub fn build_worker(
    main_module: ModuleSpecifier,
    permissions: Permissions,
    args: Vec<String>,
    reload: bool,
) -> Result<ChaqmoqWorker, anyhow::Error> {
    let mut worker = ChaqmoqWorker::new(WorkerOptions {
        main_module,
        args,
        permissions,
        reload,
    })?;

    let colors = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    worker.execute_script(
        "<chaqmoq:colors>",
        format!("globalThis.__chaqmoq.setColorsEnabled({colors});"),
    )?;

    Ok(worker)
}

/// Turns a CLI argument into a module specifier, accepting both paths and URLs.
pub fn resolve_main_module(script: &str) -> Result<ModuleSpecifier, anyhow::Error> {
    // Only treat the argument as a URL when it really looks like one, so that a
    // Windows path such as `C:\src\main.ts` is not mistaken for a scheme.
    if script.contains("://") {
        if let Ok(url) = ModuleSpecifier::parse(script) {
            return Ok(url);
        }
    }
    let cwd = std::env::current_dir()?;
    Ok(deno_core::resolve_path(script, &cwd)?)
}

pub(crate) async fn run_script(
    permissions: &PermissionFlags,
    script: &str,
    script_args: &[String],
) -> Result<i32, anyhow::Error> {
    let main_module = resolve_main_module(script)?;
    let mut worker = build_worker(
        main_module,
        permissions.to_permissions(),
        script_args.to_vec(),
        permissions.reload,
    )?;

    match worker.run_main_module().await {
        Ok(()) => Ok(0),
        Err(error) => {
            eprintln!("{}", format_core_error(&error));
            Ok(1)
        }
    }
}

async fn run_eval(
    permissions: &PermissionFlags,
    code: &str,
    typescript: bool,
) -> Result<i32, anyhow::Error> {
    let cwd = std::env::current_dir()?;
    // The extension is what tells the module loader whether to transpile.
    let name = if typescript { "$eval.ts" } else { "$eval.js" };
    let main_module = deno_core::resolve_path(name, &cwd)?;

    let mut worker = build_worker(
        main_module.clone(),
        permissions.to_permissions(),
        Vec::new(),
        permissions.reload,
    )?;

    // Inline code bypasses the module loader, so TypeScript has to be stripped
    // here rather than during loading.
    let source = if typescript {
        chaqmoq_runtime::module_loader::transpile_snippet(&main_module, code.to_string())
            .map_err(|e| anyhow::anyhow!("{e}"))?
    } else {
        code.to_string()
    };

    match worker.run_main_module_from_code(source).await {
        Ok(()) => Ok(0),
        Err(error) => {
            eprintln!("{}", format_core_error(&error));
            Ok(1)
        }
    }
}

fn print_info() {
    println!("{}", colors::bold("chaqmoq"));
    println!("  version   {}", chaqmoq_runtime::VERSION);
    println!("  v8        {}", chaqmoq_runtime::v8_version());
    println!(
        "  target    {}-{}",
        std::env::consts::ARCH,
        std::env::consts::OS
    );
    match chaqmoq_runtime::cache_dir() {
        Some(dir) => println!("  cache     {}", dir.display()),
        None => println!("  cache     <unavailable>"),
    }
    println!();
    println!("{}", colors::bold("permissions"));
    println!("  every program starts with none; grant them with --allow-* flags");
}
