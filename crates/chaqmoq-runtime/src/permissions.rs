//! Secure-by-default permission model.
//!
//! Nothing is allowed unless the user opts in on the command line. Every op that
//! touches the outside world funnels through one of the `check_*` methods below,
//! so adding a new capability without a permission check is a compile-time
//! visible omission rather than a silent hole.

use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use deno_error::JsErrorBox;

/// Error class surfaced to JavaScript when a permission check fails.
/// Matches the class `deno_core` maps onto `Deno.core.NotCapable`.
const NOT_CAPABLE: &str = "NotCapable";

/// How much of a given capability the user granted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Access {
    /// The capability is completely unavailable.
    #[default]
    Denied,
    /// The capability is available without restriction.
    Any,
    /// The capability is available, but only for the listed subjects
    /// (paths for read/write, `host[:port]` for net, variable names for env).
    List(Vec<String>),
}

impl Access {
    /// Parses a CLI flag value: `None` means the bare flag (`--allow-read`),
    /// `Some("a,b")` means the scoped form (`--allow-read=a,b`).
    pub fn from_flag(value: Option<&str>) -> Self {
        match value {
            None => Access::Any,
            Some(raw) => {
                let items: Vec<String> = raw
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
                if items.is_empty() {
                    Access::Any
                } else {
                    Access::List(items)
                }
            }
        }
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Access::Denied)
    }
}

/// The full set of capabilities granted to a program.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    pub read: Access,
    pub write: Access,
    pub net: Access,
    pub env: Access,
    pub sys: Access,
    pub run: Access,
}

impl Permissions {
    /// Everything granted. Used by `--allow-all` / `-A` and by the test runner
    /// when the user opts in.
    pub fn allow_all() -> Self {
        Self {
            read: Access::Any,
            write: Access::Any,
            net: Access::Any,
            env: Access::Any,
            sys: Access::Any,
            run: Access::Any,
        }
    }

    /// Nothing granted — the default for `chaqmoq run`.
    pub fn none() -> Self {
        Self::default()
    }

    pub fn check_read(&self, path: &Path) -> Result<(), JsErrorBox> {
        check_path(&self.read, path, "read", "--allow-read")
    }

    pub fn check_write(&self, path: &Path) -> Result<(), JsErrorBox> {
        check_path(&self.write, path, "write", "--allow-write")
    }

    pub fn check_net_url(&self, url: &url::Url) -> Result<(), JsErrorBox> {
        let host = url.host_str().unwrap_or_default();
        let port = url.port_or_known_default();
        self.check_net(host, port)
    }

    pub fn check_net(&self, host: &str, port: Option<u16>) -> Result<(), JsErrorBox> {
        let subject = match port {
            Some(p) => format!("{host}:{p}"),
            None => host.to_string(),
        };
        match &self.net {
            Access::Any => Ok(()),
            Access::Denied => Err(denied("net", &subject, "--allow-net")),
            Access::List(allowed) => {
                let ok = allowed.iter().any(|entry| {
                    // `example.com` matches any port; `example.com:443` is exact.
                    entry == &subject || entry == host
                });
                if ok {
                    Ok(())
                } else {
                    Err(denied("net", &subject, "--allow-net"))
                }
            }
        }
    }

    pub fn check_env(&self, name: &str) -> Result<(), JsErrorBox> {
        match &self.env {
            Access::Any => Ok(()),
            Access::Denied => Err(denied("env", name, "--allow-env")),
            Access::List(allowed) => {
                if allowed.iter().any(|entry| entry == name) {
                    Ok(())
                } else {
                    Err(denied("env", name, "--allow-env"))
                }
            }
        }
    }

    pub fn check_sys(&self, api: &str) -> Result<(), JsErrorBox> {
        match &self.sys {
            Access::Any => Ok(()),
            Access::Denied => Err(denied("sys", api, "--allow-sys")),
            Access::List(allowed) => {
                if allowed.iter().any(|entry| entry == api) {
                    Ok(())
                } else {
                    Err(denied("sys", api, "--allow-sys"))
                }
            }
        }
    }

    pub fn check_run(&self, cmd: &str) -> Result<(), JsErrorBox> {
        match &self.run {
            Access::Any => Ok(()),
            Access::Denied => Err(denied("run", cmd, "--allow-run")),
            Access::List(allowed) => {
                if allowed.iter().any(|entry| entry == cmd) {
                    Ok(())
                } else {
                    Err(denied("run", cmd, "--allow-run"))
                }
            }
        }
    }

    /// Human readable one-liner used by `chaqmoq info` and the REPL banner.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        for (label, access) in [
            ("read", &self.read),
            ("write", &self.write),
            ("net", &self.net),
            ("env", &self.env),
            ("sys", &self.sys),
            ("run", &self.run),
        ] {
            match access {
                Access::Denied => {}
                Access::Any => parts.push(label.to_string()),
                Access::List(items) => parts.push(format!("{label}={}", items.join(","))),
            }
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(" ")
        }
    }
}

fn check_path(access: &Access, path: &Path, kind: &str, flag: &str) -> Result<(), JsErrorBox> {
    let resolved = normalize(path);
    match access {
        Access::Any => Ok(()),
        Access::Denied => Err(denied(kind, &resolved.display().to_string(), flag)),
        Access::List(allowed) => {
            let permitted = allowed
                .iter()
                .map(|entry| normalize(Path::new(entry)))
                .any(|prefix| resolved.starts_with(&prefix));
            if permitted {
                Ok(())
            } else {
                Err(denied(kind, &resolved.display().to_string(), flag))
            }
        }
    }
}

fn denied(kind: &str, subject: &str, flag: &str) -> JsErrorBox {
    JsErrorBox::new(
        NOT_CAPABLE,
        format!("Requires {kind} access to \"{subject}\", run again with the {flag} flag"),
    )
}

/// Makes a path absolute and lexically removes `.` / `..` segments.
///
/// This deliberately does *not* touch the filesystem: `canonicalize` fails for
/// paths that do not exist yet, which is exactly the case for a write check on a
/// brand new file.
pub fn normalize(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    };

    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denied_by_default() {
        let p = Permissions::none();
        assert!(p.check_read(Path::new("/etc/passwd")).is_err());
        assert!(p.check_net("example.com", Some(443)).is_err());
        assert!(p.check_env("HOME").is_err());
    }

    #[test]
    fn allow_all_grants_everything() {
        let p = Permissions::allow_all();
        assert!(p.check_read(Path::new("/etc/passwd")).is_ok());
        assert!(p.check_write(Path::new("/tmp/x")).is_ok());
        assert!(p.check_net("example.com", Some(443)).is_ok());
    }

    #[test]
    fn scoped_read_is_prefix_matched() {
        let p = Permissions {
            read: Access::List(vec!["/tmp/project".into()]),
            ..Permissions::none()
        };
        assert!(p.check_read(Path::new("/tmp/project/a/b.txt")).is_ok());
        assert!(p.check_read(Path::new("/tmp/other")).is_err());
    }

    #[test]
    fn scoped_read_rejects_dotdot_escape() {
        let p = Permissions {
            read: Access::List(vec!["/tmp/project".into()]),
            ..Permissions::none()
        };
        assert!(p
            .check_read(Path::new("/tmp/project/../secrets.txt"))
            .is_err());
    }

    #[test]
    fn net_host_entry_matches_any_port() {
        let p = Permissions {
            net: Access::List(vec!["example.com".into()]),
            ..Permissions::none()
        };
        assert!(p.check_net("example.com", Some(8080)).is_ok());
        assert!(p.check_net("evil.com", Some(80)).is_err());
    }

    #[test]
    fn net_host_port_entry_is_exact() {
        let p = Permissions {
            net: Access::List(vec!["example.com:443".into()]),
            ..Permissions::none()
        };
        assert!(p.check_net("example.com", Some(443)).is_ok());
        assert!(p.check_net("example.com", Some(80)).is_err());
    }

    #[test]
    fn access_from_flag() {
        assert_eq!(Access::from_flag(None), Access::Any);
        assert_eq!(
            Access::from_flag(Some("a,b")),
            Access::List(vec!["a".into(), "b".into()])
        );
        assert_eq!(Access::from_flag(Some("")), Access::Any);
    }
}
