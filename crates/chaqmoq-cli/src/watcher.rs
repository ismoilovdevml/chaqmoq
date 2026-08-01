//! `chaqmoq run --watch` — re-run a program when its directory changes.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

use chaqmoq_runtime::colors;
use notify::event::EventKind;
use notify::RecursiveMode;
use notify::Watcher;

use crate::flags::PermissionFlags;

/// Editors write a file in several steps; collapsing events inside this window
/// avoids running the program two or three times per save.
const DEBOUNCE: Duration = Duration::from_millis(120);

const WATCHED_EXTENSIONS: &[&str] = &["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx", "json"];

pub fn run_with_watch(
    tokio: &tokio::runtime::Runtime,
    permissions: &PermissionFlags,
    script: &str,
    script_args: &[String],
) -> Result<i32, anyhow::Error> {
    let entry = std::fs::canonicalize(script)
        .map_err(|e| anyhow::anyhow!("could not watch {script}: {e}"))?;
    let root = entry.parent().unwrap_or(Path::new(".")).to_path_buf();

    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        // A closed receiver just means we are shutting down.
        let _ = tx.send(event);
    })?;
    watcher.watch(&root, RecursiveMode::Recursive)?;

    // macOS FSEvents coalesces with about a second of latency, so writes that
    // happened just before the watch was registered can still be delivered.
    // Discard that backlog, otherwise the first run is immediately followed by
    // a spurious second one.
    std::thread::sleep(DEBOUNCE);
    while rx.try_recv().is_ok() {}

    loop {
        clear_screen();
        println!(
            "{} {}",
            colors::cyan("Watcher"),
            format_args!("Process started ({})", relative(&entry, &root))
        );

        let exit_code = tokio.block_on(super::run_script(permissions, script, script_args))?;

        let verdict = if exit_code == 0 {
            colors::green("Process finished")
        } else {
            colors::yellow(&format!("Process exited with code {exit_code}"))
        };
        println!(
            "{} {}. Waiting for file changes...",
            colors::cyan("Watcher"),
            verdict
        );

        wait_for_change(&rx)?;
    }
}

/// Blocks until a relevant file changes, then drains the debounce window.
fn wait_for_change(
    rx: &mpsc::Receiver<notify::Result<notify::Event>>,
) -> Result<(), anyhow::Error> {
    loop {
        let event = rx.recv()?;
        let Ok(event) = event else { continue };

        if !is_relevant(&event) {
            continue;
        }

        // Swallow the burst of follow-up events from the same save.
        let deadline = Instant::now() + DEBOUNCE;
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            if rx.recv_timeout(remaining).is_err() {
                break;
            }
        }
        return Ok(());
    }
}

fn is_relevant(event: &notify::Event) -> bool {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    event.paths.iter().any(|path| {
        // Ignore editor swap files and anything that is not source.
        let is_hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        let has_source_extension = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| WATCHED_EXTENSIONS.contains(&e));
        !is_hidden && has_source_extension
    })
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn clear_screen() {
    // Only meaningful on a terminal; harmless in a pipe because the CLI's
    // colour helper is what gates the rest of the output.
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        print!("\x1b[2J\x1b[1;1H");
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use notify::event::ModifyKind;

    fn event(kind: EventKind, path: &str) -> notify::Event {
        notify::Event {
            kind,
            paths: vec![PathBuf::from(path)],
            attrs: Default::default(),
        }
    }

    #[test]
    fn source_changes_trigger_a_rerun() {
        assert!(is_relevant(&event(
            EventKind::Modify(ModifyKind::Any),
            "/src/main.ts"
        )));
    }

    #[test]
    fn non_source_and_hidden_files_are_ignored() {
        assert!(!is_relevant(&event(
            EventKind::Modify(ModifyKind::Any),
            "/src/README.md"
        )));
        assert!(!is_relevant(&event(
            EventKind::Modify(ModifyKind::Any),
            "/src/.main.ts.swp"
        )));
    }

    #[test]
    fn access_events_are_ignored() {
        assert!(!is_relevant(&event(
            EventKind::Access(notify::event::AccessKind::Read),
            "/src/main.ts"
        )));
    }
}
