//! Process, environment and platform ops behind the `Chaqmoq` namespace.

use std::collections::HashMap;
use std::path::PathBuf;

use deno_core::op2;
use deno_core::OpState;
use deno_error::JsErrorBox;
use serde::Serialize;

use super::io_error;
use crate::permissions::Permissions;
use crate::RuntimeState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub target: String,
    pub arch: String,
    pub os: String,
    pub family: String,
    pub v8: String,
    pub chaqmoq: String,
    pub rustc: String,
}

#[op2]
#[serde]
pub fn op_chq_build_info() -> BuildInfo {
    BuildInfo {
        target: format!(
            "{}-{}-{}",
            std::env::consts::ARCH,
            std::env::consts::FAMILY,
            std::env::consts::OS
        ),
        arch: std::env::consts::ARCH.to_string(),
        os: std::env::consts::OS.to_string(),
        family: std::env::consts::FAMILY.to_string(),
        v8: crate::v8_version().to_string(),
        chaqmoq: crate::VERSION.to_string(),
        rustc: env!("CARGO_PKG_RUST_VERSION").to_string(),
    }
}

#[op2]
#[serde]
pub fn op_chq_args(state: &mut OpState) -> Vec<String> {
    state.borrow::<RuntimeState>().args.clone()
}

#[op2]
#[string]
pub fn op_chq_main_module(state: &mut OpState) -> String {
    state.borrow::<RuntimeState>().main_module.clone()
}

#[op2(fast)]
pub fn op_chq_pid() -> u32 {
    std::process::id()
}

#[op2]
#[string]
pub fn op_chq_cwd(state: &mut OpState) -> Result<String, JsErrorBox> {
    let cwd = std::env::current_dir().map_err(|e| io_error(e, "cwd"))?;
    // Reading the CWD leaks filesystem layout, so it is gated like any read.
    state.borrow::<Permissions>().check_read(&cwd)?;
    Ok(cwd.to_string_lossy().into_owned())
}

#[op2(fast)]
pub fn op_chq_chdir(state: &mut OpState, #[string] path: String) -> Result<(), JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_read(&path)?;
    std::env::set_current_dir(&path).map_err(|e| io_error(e, &path.display().to_string()))
}

#[op2(fast)]
pub fn op_chq_exit(code: i32) {
    // Flushing matters because stdout is line buffered when piped.
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::process::exit(code);
}

// ---------------------------------------------------------------------------
// Environment variables
// ---------------------------------------------------------------------------

#[op2]
#[string]
pub fn op_chq_env_get(
    state: &mut OpState,
    #[string] key: String,
) -> Result<Option<String>, JsErrorBox> {
    state.borrow::<Permissions>().check_env(&key)?;
    Ok(std::env::var(&key).ok())
}

#[op2(fast)]
pub fn op_chq_env_set(
    state: &mut OpState,
    #[string] key: String,
    #[string] value: String,
) -> Result<(), JsErrorBox> {
    state.borrow::<Permissions>().check_env(&key)?;
    if key.is_empty() || key.contains('=') || key.contains('\0') {
        return Err(JsErrorBox::type_error(format!(
            "invalid environment variable name: {key:?}"
        )));
    }
    // SAFETY: Chaqmoq drives V8 on a single thread and does not expose threads
    // to user code, so there is no concurrent reader of the environment.
    unsafe { std::env::set_var(&key, &value) };
    Ok(())
}

#[op2(fast)]
pub fn op_chq_env_delete(state: &mut OpState, #[string] key: String) -> Result<(), JsErrorBox> {
    state.borrow::<Permissions>().check_env(&key)?;
    // SAFETY: see `op_chq_env_set`.
    unsafe { std::env::remove_var(&key) };
    Ok(())
}

#[op2]
#[serde]
pub fn op_chq_env_to_object(state: &mut OpState) -> Result<HashMap<String, String>, JsErrorBox> {
    let permissions = state.borrow::<Permissions>().clone();
    let mut out = HashMap::new();
    for (key, value) in std::env::vars() {
        // With a scoped `--allow-env=A,B` only the listed keys are visible,
        // rather than the whole call failing.
        if permissions.check_env(&key).is_ok() {
            out.insert(key, value);
        }
    }
    if out.is_empty() && permissions.env.is_denied() {
        return Err(JsErrorBox::new(
            "NotCapable",
            "Requires env access, run again with the --allow-env flag",
        ));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// System information
// ---------------------------------------------------------------------------

#[op2]
#[string]
pub fn op_chq_hostname(state: &mut OpState) -> Result<String, JsErrorBox> {
    state.borrow::<Permissions>().check_sys("hostname")?;
    Ok(hostname())
}

fn hostname() -> String {
    // `HOSTNAME`/`COMPUTERNAME` cover the common cases without pulling in a
    // platform crate; `uname -n` is the portable fallback on unix.
    if let Ok(name) = std::env::var("HOSTNAME") {
        if !name.is_empty() {
            return name;
        }
    }
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        if !name.is_empty() {
            return name;
        }
    }
    #[cfg(unix)]
    {
        if let Ok(output) = std::process::Command::new("uname").arg("-n").output() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    "unknown".to_string()
}

#[op2(fast)]
pub fn op_chq_uptime(state: &mut OpState) -> f64 {
    state.borrow::<RuntimeState>().start.elapsed().as_secs_f64()
}

/// Backs `performance.now()`: milliseconds, sub-millisecond precision, measured
/// from the moment the runtime was created.
#[op2(fast)]
pub fn op_chq_now(state: &mut OpState) -> f64 {
    state.borrow::<RuntimeState>().start.elapsed().as_secs_f64() * 1000.0
}

#[op2(fast)]
pub fn op_chq_time_origin(state: &mut OpState) -> f64 {
    state.borrow::<RuntimeState>().time_origin
}
