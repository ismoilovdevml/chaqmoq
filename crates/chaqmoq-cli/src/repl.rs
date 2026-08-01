//! Interactive read-eval-print loop.

use chaqmoq_runtime::colors;
use chaqmoq_runtime::errors::format_core_error;
use chaqmoq_runtime::errors::format_js_error;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::flags::PermissionFlags;

/// Installed once, then called with each line of input. Keeping the evaluation
/// in JavaScript means results are rendered by the same inspector `console.log`
/// uses, and `let` / `const` bindings survive between lines because
/// `evalContext` shares the main context.
const REPL_PRELUDE: &str = r#"
globalThis.__replEval = (source) => {
  const SPECIFIER = "file:///$repl.js";
  const inspect = globalThis.__chaqmoq.inspect;

  const report = (thrown) => {
    if (thrown instanceof Error) console.error(thrown.stack ?? String(thrown));
    else console.error("Uncaught", inspect(thrown));
  };

  const show = (value) => {
    if (value === undefined) return;
    // Awaiting thenables makes `fetch(...)` and friends print their result
    // rather than a pending promise.
    if (value !== null && typeof value.then === "function") {
      return value.then((settled) => show(settled), report);
    }
    console.log(inspect(value));
  };

  let [result, error] = Deno.core.evalContext(source, SPECIFIER);

  // `evalContext` shares the main context, which keeps `let`/`const` alive
  // between lines but forbids top-level await. Retry inside an async wrapper:
  // first as an expression (so the value is still printed), then as a block.
  if (error && String(error.thrown).includes("await is only valid")) {
    for (const wrapped of [
      `(async () => (\n${source}\n))()`,
      `(async () => {\n${source}\n})()`,
    ]) {
      [result, error] = Deno.core.evalContext(wrapped, SPECIFIER);
      if (!error) break;
    }
  }

  if (error) {
    report(error.thrown);
    return;
  }
  return show(result);
};
"#;

pub async fn start(permissions: &PermissionFlags) -> Result<i32, anyhow::Error> {
    let cwd = std::env::current_dir()?;
    let main_module = deno_core::resolve_path("$repl.js", &cwd)?;

    let mut worker = crate::build_worker(
        main_module,
        permissions.to_permissions(),
        Vec::new(),
        permissions.reload,
    )?;

    // An empty main module gives the REPL a fully bootstrapped global scope.
    worker.run_main_module_from_code(String::new()).await?;
    worker.execute_script("<chaqmoq:repl>", REPL_PRELUDE)?;

    println!(
        "Chaqmoq {} ({})",
        chaqmoq_runtime::VERSION,
        colors::dim(&format!("v8 {}", chaqmoq_runtime::v8_version()))
    );
    println!(
        "{}",
        colors::dim(&format!(
            "permissions: {} — exit with ctrl+d, ctrl+c or .exit",
            permissions.to_permissions().summary()
        ))
    );

    let mut editor = DefaultEditor::new()?;
    let mut buffer = String::new();

    loop {
        let prompt = if buffer.is_empty() { "> " } else { "  " };
        match editor.readline(prompt) {
            Ok(line) => {
                let trimmed = line.trim();
                if buffer.is_empty() && matches!(trimmed, ".exit" | ".quit") {
                    break;
                }
                if buffer.is_empty() && trimmed.is_empty() {
                    continue;
                }

                buffer.push_str(&line);
                buffer.push('\n');

                // Keep collecting lines while the input is an unfinished
                // block, so multi-line functions and objects work.
                if is_incomplete(&buffer) {
                    continue;
                }

                let source = std::mem::take(&mut buffer);
                let _ = editor.add_history_entry(source.trim_end());

                let script = format!(
                    "globalThis.__replEval({});",
                    serde_json::to_string(source.trim_end())?
                );
                if let Err(error) = worker.execute_script("<repl>", script) {
                    eprintln!("{}", format_js_error(&error));
                    continue;
                }

                // Let timers, promises and I/O started by the line finish.
                if let Err(error) = worker.run_event_loop().await {
                    eprintln!("{}", format_core_error(&error));
                }
            }
            Err(ReadlineError::Interrupted) => {
                if buffer.is_empty() {
                    break;
                }
                // Ctrl+C during a continuation abandons the partial input.
                buffer.clear();
            }
            Err(ReadlineError::Eof) => break,
            Err(error) => return Err(error.into()),
        }
    }

    Ok(0)
}

/// Cheap bracket balance check — enough to tell "still typing a block" from
/// "ready to evaluate" without embedding a parser.
fn is_incomplete(source: &str) -> bool {
    let mut depth = 0i32;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_template = false;
    let mut escaped = false;

    for ch in source.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_single || in_double || in_template => escaped = true,
            '\'' if !in_double && !in_template => in_single = !in_single,
            '"' if !in_single && !in_template => in_double = !in_double,
            '`' if !in_single && !in_double => in_template = !in_template,
            '(' | '[' | '{' if !in_single && !in_double && !in_template => depth += 1,
            ')' | ']' | '}' if !in_single && !in_double && !in_template => depth -= 1,
            _ => {}
        }
    }

    depth > 0 || in_template
}

#[cfg(test)]
mod tests {
    use super::is_incomplete;

    #[test]
    fn complete_expressions_evaluate_immediately() {
        assert!(!is_incomplete("1 + 1"));
        assert!(!is_incomplete("const a = { b: 1 };"));
        assert!(!is_incomplete("f({ a: [1, 2] })"));
    }

    #[test]
    fn open_blocks_continue() {
        assert!(is_incomplete("function f() {"));
        assert!(is_incomplete("const a = ["));
    }

    #[test]
    fn brackets_inside_strings_are_ignored() {
        assert!(!is_incomplete("\"{\""));
        assert!(!is_incomplete("'([{'"));
        assert!(!is_incomplete("\"\\\"{\""));
    }

    #[test]
    fn unterminated_template_continues() {
        assert!(is_incomplete("`hello"));
        assert!(!is_incomplete("`hello`"));
    }
}
