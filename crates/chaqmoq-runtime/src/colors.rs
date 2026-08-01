//! Tiny ANSI helper.
//!
//! Colour is disabled when stderr is not a TTY, when `NO_COLOR` is set (per
//! <https://no-color.org>), or when `TERM=dumb` — so piping output into a file
//! or a CI log stays clean.

use std::io::IsTerminal;
use std::sync::OnceLock;

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        if std::env::var_os("NO_COLOR").is_some() {
            return false;
        }
        if matches!(std::env::var("TERM").as_deref(), Ok("dumb")) {
            return false;
        }
        std::io::stderr().is_terminal()
    })
}

fn paint(code: &str, text: &str) -> String {
    if enabled() {
        format!("\u{1b}[{code}m{text}\u{1b}[0m")
    } else {
        text.to_string()
    }
}

pub fn red(text: &str) -> String {
    paint("31", text)
}

pub fn red_bold(text: &str) -> String {
    paint("1;31", text)
}

pub fn green(text: &str) -> String {
    paint("32", text)
}

pub fn green_bold(text: &str) -> String {
    paint("1;32", text)
}

pub fn yellow(text: &str) -> String {
    paint("33", text)
}

pub fn blue(text: &str) -> String {
    paint("34", text)
}

pub fn cyan(text: &str) -> String {
    paint("36", text)
}

pub fn bold(text: &str) -> String {
    paint("1", text)
}

pub fn dim(text: &str) -> String {
    paint("2", text)
}
