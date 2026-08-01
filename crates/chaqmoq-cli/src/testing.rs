//! `chaqmoq test` — discovery and execution of `Chaqmoq.test` registrations.

use std::io::IsTerminal;
use std::path::Path;
use std::path::PathBuf;

use chaqmoq_runtime::colors;
use chaqmoq_runtime::errors::format_core_error;

use crate::flags::PermissionFlags;

/// A file is a test file when its stem ends in `_test` or `.test`, or when it
/// is literally named `test.<ext>` — the same convention Deno uses.
fn is_test_file(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    if !matches!(extension, "js" | "mjs" | "ts" | "mts" | "jsx" | "tsx") {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    stem == "test" || stem.ends_with("_test") || stem.ends_with(".test")
}

/// Directories that are never worth walking.
fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules" | ".git" | "target" | "dist" | "build" | ".chaqmoq"
    )
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let metadata = std::fs::metadata(path)?;

    if metadata.is_file() {
        // An explicitly named file is always included, test-shaped or not.
        out.push(path.to_path_buf());
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(path)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let child = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if child.is_dir() {
            if !is_ignored_dir(&name) && !name.starts_with('.') {
                collect(&child, out)?;
            }
        } else if is_test_file(&child) {
            out.push(child);
        }
    }
    Ok(())
}

pub async fn run_tests(
    permissions: &PermissionFlags,
    paths: &[String],
    filter: Option<&str>,
) -> Result<i32, anyhow::Error> {
    let cwd = std::env::current_dir()?;
    let roots: Vec<PathBuf> = if paths.is_empty() {
        vec![cwd.clone()]
    } else {
        paths.iter().map(PathBuf::from).collect()
    };

    let mut files = Vec::new();
    for root in &roots {
        collect(root, &mut files)
            .map_err(|e| anyhow::anyhow!("could not read {}: {e}", root.display()))?;
    }
    files.sort();
    files.dedup();

    if files.is_empty() {
        eprintln!(
            "{}: no test files found. Name them `*_test.ts` or `*.test.ts`.",
            colors::yellow("warning")
        );
        return Ok(0);
    }

    println!(
        "running tests from {} file{}",
        files.len(),
        if files.len() == 1 { "" } else { "s" }
    );

    // A synthetic main module imports every test file, so registration happens
    // through the normal module graph — including TypeScript transpilation.
    let mut source = String::new();
    for file in &files {
        let specifier = deno_core::resolve_path(&file.to_string_lossy(), &cwd)?;
        source.push_str(&format!("import {};\n", json_string(specifier.as_str())));
    }

    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let filter_literal = match filter {
        Some(value) => json_string(value),
        None => "undefined".to_string(),
    };
    source.push_str(&format!(
        "const __results = await globalThis.__chaqmoq.runRegisteredTests({{ \
           color: {color}, filter: {filter_literal} }});\n\
         if (__results.failed > 0) Chaqmoq.exit(1);\n"
    ));

    let main_module = deno_core::resolve_path("$test.js", &cwd)?;
    let mut worker = crate::build_worker(
        main_module,
        permissions.to_permissions(),
        Vec::new(),
        permissions.reload,
    )?;

    match worker.run_main_module_from_code(source).await {
        Ok(()) => Ok(0),
        Err(error) => {
            eprintln!("{}", format_core_error(&error));
            Ok(1)
        }
    }
}

/// JSON is a subset of JavaScript string syntax, so this is a safe literal.
fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("strings always serialise")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_test_files() {
        assert!(is_test_file(Path::new("a_test.ts")));
        assert!(is_test_file(Path::new("a.test.js")));
        assert!(is_test_file(Path::new("dir/test.tsx")));
        assert!(!is_test_file(Path::new("main.ts")));
        assert!(!is_test_file(Path::new("a_test.txt")));
        assert!(!is_test_file(Path::new("testing.ts")));
    }

    #[test]
    fn skips_vendor_directories() {
        assert!(is_ignored_dir("node_modules"));
        assert!(is_ignored_dir("target"));
        assert!(!is_ignored_dir("src"));
    }
}
