//! End-to-end tests: they run the real `chaqmoq` binary against real files.

use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

/// Absolute path to the binary cargo just built for this test run.
const BIN: &str = env!("CARGO_BIN_EXE_chaqmoq");

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is <root>/crates/chaqmoq-cli.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate lives two levels below the workspace root")
        .to_path_buf()
}

fn chaqmoq(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(repo_root())
        // Deterministic output regardless of whether tests run on a TTY.
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to launch the chaqmoq binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Writes a script into a unique temp directory and returns its path.
fn script(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chaqmoq_it_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("could not create the test directory");
    let path = dir.join(name);
    std::fs::write(&path, source).expect("could not write the test script");
    path
}

// ---------------------------------------------------------------------------
// Basics
// ---------------------------------------------------------------------------

#[test]
fn runs_a_javascript_file() {
    let path = script("basic.js", "console.log('hello from js');\n");
    let output = chaqmoq(&["run", path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "hello from js");
}

#[test]
fn transpiles_and_runs_typescript() {
    let path = script(
        "typed.ts",
        "interface P { n: number }\nconst p: P = { n: 7 };\nconsole.log(p.n * 6);\n",
    );
    let output = chaqmoq(&["run", path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "42");
}

#[test]
fn evaluates_inline_code() {
    let output = chaqmoq(&["eval", "console.log([1,2,3].reduce((a,b)=>a+b))"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "6");
}

#[test]
fn evaluates_inline_typescript() {
    let output = chaqmoq(&["eval", "--ts", "const n: number = 21; console.log(n * 2)"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "42");
}

#[test]
fn reports_the_runtime_version() {
    let output = chaqmoq(&["info"]);

    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("chaqmoq"), "info output was: {text}");
    assert!(text.contains("v8"), "info output was: {text}");
}

// ---------------------------------------------------------------------------
// Permissions
// ---------------------------------------------------------------------------

#[test]
fn file_reads_are_denied_without_the_flag() {
    let path = script(
        "read_denied.js",
        "await Chaqmoq.readTextFile('/etc/hosts');\n",
    );
    let output = chaqmoq(&["run", path.to_str().unwrap()]);

    assert!(!output.status.success(), "the program should have failed");
    let text = stderr(&output);
    assert!(text.contains("--allow-read"), "stderr was: {text}");
}

#[test]
fn file_reads_succeed_with_the_flag() {
    let target = script("readable.txt", "chaqmoq\n");
    let path = script(
        "read_allowed.js",
        &format!(
            "console.log((await Chaqmoq.readTextFile({})).trim());\n",
            serde_json::to_string(target.to_str().unwrap()).unwrap()
        ),
    );
    let output = chaqmoq(&["run", "--allow-read", path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "chaqmoq");
}

#[test]
fn scoped_read_permission_rejects_paths_outside_the_scope() {
    let dir = std::env::temp_dir().join(format!("chaqmoq_scope_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = script("scoped.js", "await Chaqmoq.readTextFile('/etc/hosts');\n");

    let flag = format!("--allow-read={}", dir.display());
    let output = chaqmoq(&["run", &flag, path.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("--allow-read"));
}

#[test]
fn environment_access_is_denied_without_the_flag() {
    let output = chaqmoq(&["eval", "Chaqmoq.env.get('PATH')"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("--allow-env"));
}

#[test]
fn scoped_environment_access_only_exposes_named_variables() {
    let output = chaqmoq(&[
        "eval",
        "--allow-env=CHAQMOQ_TEST_VAR",
        "console.log(typeof Chaqmoq.env.get('CHAQMOQ_TEST_VAR'))",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    // Unset, so `null` — but crucially it did not throw.
    assert_eq!(stdout(&output).trim(), "object");
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[test]
fn a_thrown_error_exits_non_zero_with_a_stack_trace() {
    let path = script(
        "throws.ts",
        "function boom(): never { throw new Error('kaboom'); }\nboom();\n",
    );
    let output = chaqmoq(&["run", path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    let text = stderr(&output);
    assert!(text.contains("kaboom"), "stderr was: {text}");
    // The frame must point at the TypeScript source, not the generated JS.
    assert!(text.contains("throws.ts:1:"), "stderr was: {text}");
}

#[test]
fn a_bare_specifier_gets_an_actionable_message() {
    let path = script("bare.js", "import 'lodash';\n");
    let output = chaqmoq(&["run", path.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("not prefixed with"),
        "stderr was: {}",
        stderr(&output)
    );
}

#[test]
fn chaqmoq_exit_sets_the_status_code() {
    let output = chaqmoq(&["eval", "Chaqmoq.exit(3)"]);
    assert_eq!(output.status.code(), Some(3));
}

// ---------------------------------------------------------------------------
// Module graph
// ---------------------------------------------------------------------------

#[test]
fn imports_json_modules() {
    let data = script("data.json", r#"{"answer": 42}"#);
    let path = script(
        "json_import.ts",
        &format!(
            "import data from {} with {{ type: \"json\" }};\nconsole.log(data.answer);\n",
            serde_json::to_string(&format!(
                "./{}",
                data.file_name().unwrap().to_string_lossy()
            ))
            .unwrap()
        ),
    );
    let output = chaqmoq(&["run", path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "42");
}

#[test]
fn imports_relative_typescript_modules() {
    script("dep.ts", "export const value = 'from dep';\n");
    let path = script(
        "importer.ts",
        "import { value } from './dep.ts';\nconsole.log(value);\n",
    );
    let output = chaqmoq(&["run", path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "from dep");
}

// ---------------------------------------------------------------------------
// Test runner
// ---------------------------------------------------------------------------

#[test]
fn the_test_runner_passes_on_the_bundled_suite() {
    let output = chaqmoq(&["test", "examples/tests"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("test result: ok"), "stdout was: {text}");
}

#[test]
fn the_test_runner_exits_non_zero_on_failure() {
    let path = script(
        "failing_test.ts",
        "Chaqmoq.test('fails', () => { throw new Error('nope'); });\n",
    );
    let output = chaqmoq(&["test", path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("FAILED"));
}

#[test]
fn the_test_runner_filters_by_name() {
    let path = script(
        "filtered_test.ts",
        "Chaqmoq.test('alpha', () => {});\nChaqmoq.test('beta', () => {});\n",
    );
    let output = chaqmoq(&["test", "--filter", "alpha", path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("alpha"), "stdout was: {text}");
    assert!(!text.contains("beta"), "stdout was: {text}");
}

// ---------------------------------------------------------------------------
// Examples
// ---------------------------------------------------------------------------

#[test]
fn the_hello_example_runs() {
    let output = chaqmoq(&["run", "examples/hello.ts"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("Salom, Chaqmoq!"));
}

#[test]
fn the_showcase_example_runs() {
    let output = chaqmoq(&["run", "examples/showcase.ts"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("[Circular *1]"), "stdout was: {text}");
    assert!(text.contains("Map(2)"), "stdout was: {text}");
}

#[test]
fn the_filesystem_example_runs_with_permissions() {
    let output = chaqmoq(&[
        "run",
        "--allow-read",
        "--allow-write",
        "examples/filesystem.ts",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("cleaned up: true"));
}
