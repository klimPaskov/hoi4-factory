//! Hardened `git` invocations for the GitHub import.
//!
//! `factory-workspace` owns local Git work and never touches the network. Import needs a
//! fetch, so this module runs `git` with the same hardening and one addition: a
//! per-operation HTTP credential header passed through `GIT_CONFIG_COUNT` environment
//! variables. The header exists only in the child's environment. It is never written to a
//! configuration file, an argument list, a log line or an error message.
//!
//! Hardening, as in `factory-workspace`: system and global configuration are not read,
//! hooks are disabled, `core.autocrlf=false` and `core.safecrlf=false` are forced, every
//! inherited `GIT_*` variable is dropped, and the terminal prompt is off. Added here:
//! only the transport the endpoint needs is allowed, and the credential helper is empty.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use tokio::process::Command;

use crate::error::IntakeError;
use crate::secrets::SecretString;
use crate::util::scrub;

/// Credential for one Git operation against one base URL.
pub(crate) struct GitAuth {
    /// `http.<base>/.extraheader`
    key: String,
    /// `AUTHORIZATION: basic <base64>`
    value: SecretString,
    /// The raw token, for scrubbing captured output.
    token: SecretString,
    /// Allow plain `http` (loopback test servers only).
    allow_http: bool,
}

impl GitAuth {
    pub(crate) fn new(clone_base: &str, token: &str) -> Self {
        let encoded = STANDARD.encode(format!("x-access-token:{token}"));
        GitAuth {
            key: format!("http.{}/.extraheader", clone_base.trim_end_matches('/')),
            value: SecretString::new(format!("AUTHORIZATION: basic {encoded}")),
            token: SecretString::new(token),
            allow_http: clone_base.starts_with("http://"),
        }
    }
}

impl std::fmt::Debug for GitAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GitAuth([redacted])")
    }
}

pub(crate) struct GitOutput {
    pub stdout: Vec<u8>,
}

/// Run `git <args>` in `cwd`. A non-zero exit is an error whose text has the credential
/// removed. `cwd` must exist.
pub(crate) async fn run(
    cwd: &Path,
    args: &[&str],
    auth: Option<&GitAuth>,
    timeout: Duration,
) -> Result<GitOutput, GitRunError> {
    let mut command = Command::new("git");
    command.current_dir(cwd);
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    let ceiling = cwd
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cwd.to_path_buf());
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CEILING_DIRECTORIES", ceiling)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(auth) = auth {
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", &auth.key)
            .env("GIT_CONFIG_VALUE_0", auth.value.expose());
    }
    command
        .arg("--no-optional-locks")
        .arg("--literal-pathspecs")
        .args(["-c", "core.autocrlf=false"])
        .args(["-c", "core.safecrlf=false"])
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(["-c", "core.fsmonitor=false"])
        .args(["-c", "core.quotepath=false"])
        .args(["-c", "core.longpaths=true"])
        .args(["-c", "credential.helper="])
        .args(["-c", "protocol.allow=never"])
        .args(["-c", "protocol.https.allow=always"])
        .args(["-c", "protocol.ext.allow=never"])
        .args(["-c", "gc.auto=0"])
        .args(["-c", "maintenance.auto=false"])
        .args(["-c", "core.editor=true"])
        .args(["-c", "advice.detachedHead=false"]);
    if auth.is_some_and(|a| a.allow_http) {
        command.args(["-c", "protocol.http.allow=always"]);
    }
    command.args(args);

    let output = match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => return Err(GitRunError::Spawn(err)),
        Err(_) => return Err(GitRunError::TimedOut),
    };
    if output.status.success() {
        return Ok(GitOutput {
            stdout: output.stdout,
        });
    }
    let mut secrets: Vec<&str> = Vec::new();
    if let Some(auth) = auth {
        secrets.push(auth.token.expose());
        secrets.push(auth.value.expose());
    }
    let stderr = scrub(&String::from_utf8_lossy(&output.stderr), &secrets);
    Err(GitRunError::Failed {
        code: output.status.code(),
        stderr,
    })
}

#[derive(Debug)]
pub(crate) enum GitRunError {
    Spawn(std::io::Error),
    TimedOut,
    Failed { code: Option<i32>, stderr: String },
}

impl GitRunError {
    /// Whether git's output suggests the remote refused the credential.
    pub(crate) fn looks_unauthorized(&self) -> bool {
        match self {
            GitRunError::Failed { stderr, .. } => {
                let lower = stderr.to_ascii_lowercase();
                // Git never prompts here, so a rejected credential surfaces as a refused
                // prompt (401), an HTTP error (403) or GitHub's "repository not found" (404).
                lower.contains("401")
                    || lower.contains("403")
                    || lower.contains("authentication failed")
                    || lower.contains("terminal prompts disabled")
                    || lower.contains("could not read username")
                    || (lower.contains("not found") && lower.contains("repository"))
            }
            _ => false,
        }
    }
}

impl From<GitRunError> for IntakeError {
    fn from(err: GitRunError) -> Self {
        match err {
            GitRunError::Spawn(source) => {
                IntakeError::Git(factory_workspace::GitError::Spawn(source))
            }
            GitRunError::TimedOut => {
                IntakeError::GithubUnavailable("git operation timed out".into())
            }
            GitRunError::Failed { code, stderr } => {
                tracing::warn!(?code, stderr = %stderr, "git operation failed");
                IntakeError::CloneFailed
            }
        }
    }
}
