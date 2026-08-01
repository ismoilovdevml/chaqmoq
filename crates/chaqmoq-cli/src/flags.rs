//! Command line surface.

use chaqmoq_runtime::permissions::Access;
use chaqmoq_runtime::permissions::Permissions;
use clap::Args;
use clap::Parser;
use clap::Subcommand;

const LONG_ABOUT: &str = "\
Chaqmoq is a JavaScript and TypeScript runtime built on V8 and Rust.

Programs start with no access to the filesystem, the network or the
environment. Grant exactly what a program needs with the --allow-* flags.

  chaqmoq run --allow-read=. build.ts
  chaqmoq run --allow-net=0.0.0.0:8000 server.ts
  chaqmoq test --allow-read tests/";

#[derive(Parser, Debug)]
#[command(
    name = "chaqmoq",
    version,
    about = "A fast, secure-by-default JavaScript & TypeScript runtime",
    long_about = LONG_ABOUT,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run a JavaScript or TypeScript program
    Run {
        #[command(flatten)]
        permissions: PermissionFlags,

        /// Re-run the program whenever a watched file changes
        #[arg(long)]
        watch: bool,

        /// Path or URL of the module to run
        script: String,

        /// Arguments passed through to the program as `Chaqmoq.args`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        script_args: Vec<String>,
    },

    /// Evaluate a snippet of code
    Eval {
        #[command(flatten)]
        permissions: PermissionFlags,

        /// Treat the snippet as TypeScript
        #[arg(long)]
        ts: bool,

        /// The code to evaluate
        code: String,
    },

    /// Start an interactive read-eval-print loop
    Repl {
        #[command(flatten)]
        permissions: PermissionFlags,
    },

    /// Run tests registered with `Chaqmoq.test`
    Test {
        #[command(flatten)]
        permissions: PermissionFlags,

        /// Only run tests whose name contains this substring
        #[arg(long, value_name = "SUBSTRING")]
        filter: Option<String>,

        /// Files or directories to search (defaults to the working directory)
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
    },

    /// Show runtime, V8 and cache information
    Info,
}

/// The `--allow-*` family, shared by every subcommand that runs code.
///
/// Each flag works bare (`--allow-read`, meaning "everything") or scoped
/// (`--allow-read=src,tests`).
#[derive(Args, Debug, Clone, Default)]
pub struct PermissionFlags {
    /// Grant every permission — convenient for local scripts, never for
    /// untrusted code
    #[arg(short = 'A', long = "allow-all")]
    pub allow_all: bool,

    /// Allow reading files, optionally limited to the given paths
    #[arg(long = "allow-read", value_name = "PATHS", num_args = 0..=1,
          require_equals = true, default_missing_value = "")]
    pub allow_read: Option<String>,

    /// Allow writing files, optionally limited to the given paths
    #[arg(long = "allow-write", value_name = "PATHS", num_args = 0..=1,
          require_equals = true, default_missing_value = "")]
    pub allow_write: Option<String>,

    /// Allow network access, optionally limited to `host` or `host:port`
    #[arg(long = "allow-net", value_name = "HOSTS", num_args = 0..=1,
          require_equals = true, default_missing_value = "")]
    pub allow_net: Option<String>,

    /// Allow reading environment variables, optionally limited to given names
    #[arg(long = "allow-env", value_name = "NAMES", num_args = 0..=1,
          require_equals = true, default_missing_value = "")]
    pub allow_env: Option<String>,

    /// Allow querying system information such as the hostname
    #[arg(long = "allow-sys", value_name = "APIS", num_args = 0..=1,
          require_equals = true, default_missing_value = "")]
    pub allow_sys: Option<String>,

    /// Allow running subprocesses (reserved; no API uses it yet)
    #[arg(long = "allow-run", value_name = "COMMANDS", num_args = 0..=1,
          require_equals = true, default_missing_value = "")]
    pub allow_run: Option<String>,

    /// Ignore the cache and re-download remote modules
    #[arg(long)]
    pub reload: bool,
}

impl PermissionFlags {
    pub fn to_permissions(&self) -> Permissions {
        if self.allow_all {
            return Permissions::allow_all();
        }
        Permissions {
            read: access(&self.allow_read),
            write: access(&self.allow_write),
            net: access(&self.allow_net),
            env: access(&self.allow_env),
            sys: access(&self.allow_sys),
            run: access(&self.allow_run),
        }
    }
}

/// An absent flag is a denial; a present one is either blanket or scoped.
fn access(flag: &Option<String>) -> Access {
    match flag {
        None => Access::Denied,
        Some(value) => Access::from_flag(Some(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_flags_denies_everything() {
        let permissions = PermissionFlags::default().to_permissions();
        assert_eq!(permissions.summary(), "none");
    }

    #[test]
    fn allow_all_wins_over_scoped_flags() {
        let flags = PermissionFlags {
            allow_all: true,
            allow_read: Some("src".into()),
            ..Default::default()
        };
        assert_eq!(flags.to_permissions().read, Access::Any);
    }

    #[test]
    fn bare_flag_is_blanket_access() {
        let flags = PermissionFlags {
            allow_read: Some(String::new()),
            ..Default::default()
        };
        assert_eq!(flags.to_permissions().read, Access::Any);
    }

    #[test]
    fn scoped_flag_is_parsed_as_a_list() {
        let flags = PermissionFlags {
            allow_net: Some("example.com,localhost:8000".into()),
            ..Default::default()
        };
        assert_eq!(
            flags.to_permissions().net,
            Access::List(vec!["example.com".into(), "localhost:8000".into()])
        );
    }

    #[test]
    fn cli_parses_run_with_trailing_script_args() {
        let cli = Cli::parse_from([
            "chaqmoq",
            "run",
            "--allow-read=.",
            "main.ts",
            "--port",
            "3000",
        ]);
        match cli.command {
            Command::Run {
                script,
                script_args,
                permissions,
                ..
            } => {
                assert_eq!(script, "main.ts");
                assert_eq!(script_args, vec!["--port", "3000"]);
                assert_eq!(
                    permissions.to_permissions().read,
                    Access::List(vec![".".into()])
                );
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }
}
