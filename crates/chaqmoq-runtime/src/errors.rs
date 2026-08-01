//! Turning a thrown JavaScript value into something a human wants to read.

use deno_core::error::CoreError;
use deno_core::error::JsError;
use deno_core::error::JsStackFrame;

use crate::colors;

/// Formats an error the way the CLI prints it: message, caret-underlined source
/// line, then the stack, then any `cause` chain.
pub fn format_js_error(error: &JsError) -> String {
    let mut out = String::new();
    out.push_str(&colors::red_bold("error"));
    out.push_str(": ");
    out.push_str(&error.exception_message);
    out.push('\n');

    if let Some(source_line) = &error.source_line {
        // Only worth showing when there is a frame to anchor the caret to.
        if let Some(index) = error.source_line_frame_index {
            if let Some(frame) = error.frames.get(index) {
                out.push_str(&format_source_line(source_line, frame));
            }
        }
    }

    for frame in &error.frames {
        out.push_str(&format!("    at {}\n", colors::dim(&format_frame(frame))));
    }

    let mut cause = error.cause.as_deref();
    while let Some(inner) = cause {
        out.push_str(&format!(
            "{}: {}\n",
            colors::yellow("Caused by"),
            inner.exception_message
        ));
        for frame in &inner.frames {
            out.push_str(&format!("    at {}\n", colors::dim(&format_frame(frame))));
        }
        cause = inner.cause.as_deref();
    }

    if let Some(aggregated) = &error.aggregated {
        for inner in aggregated {
            out.push_str(&indent(&format_js_error(inner)));
        }
    }

    out
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("  {line}\n"))
        .collect::<String>()
}

fn format_source_line(source_line: &str, frame: &JsStackFrame) -> String {
    // A minified bundle on one line is noise, not context.
    if source_line.trim().is_empty() || source_line.len() > 300 {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(source_line);
    out.push('\n');

    if let Some(column) = frame.column_number {
        let column = column.max(1) as usize;
        let indent: String = source_line
            .chars()
            .take(column - 1)
            // Preserve tabs so the caret lines up in the terminal.
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .collect();
        out.push_str(&indent);
        out.push_str(&colors::red("^"));
        out.push('\n');
    }
    out
}

fn format_frame(frame: &JsStackFrame) -> String {
    let location = match (&frame.file_name, frame.line_number, frame.column_number) {
        (Some(file), Some(line), Some(column)) => format!("{file}:{line}:{column}"),
        (Some(file), Some(line), None) => format!("{file}:{line}"),
        (Some(file), None, None) => file.clone(),
        _ if frame.is_native => "native".to_string(),
        _ => "<anonymous>".to_string(),
    };

    match &frame.function_name {
        Some(name) if !name.is_empty() => {
            let prefix = if frame.is_async { "async " } else { "" };
            format!("{prefix}{name} ({location})")
        }
        _ => location,
    }
}

/// Best-effort pretty printer for anything that escapes the runtime.
pub fn format_error(error: &anyhow::Error) -> String {
    if let Some(core) = error.downcast_ref::<CoreError>() {
        return format_core_error(core);
    }
    if let Some(js) = error.downcast_ref::<JsError>() {
        return format_js_error(js);
    }
    format!("{}: {error}", colors::red_bold("error"))
}

pub fn format_core_error(error: &CoreError) -> String {
    // `CoreError` derefs to its kind; JS exceptions are the case worth
    // unwrapping, everything else reads fine as a plain message.
    let text = error.to_string();
    if let deno_core::error::CoreErrorKind::Js(js) = error.as_kind() {
        return format_js_error(js);
    }
    format!("{}: {text}", colors::red_bold("error"))
}
