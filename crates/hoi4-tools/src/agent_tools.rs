//! The launch spec for HOI4 Agent Tools.
//!
//! The server runs as a supervised stdio child whose working directory is the mod (the task
//! worktree). Its environment is an explicit allowlist: a fixed `PATH`, a home and four XDG
//! directories under the per-task state directory, the optional game folder and two fixed
//! settings. No tokens, API keys or host variables are ever passed.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::mcp::McpServerSpec;

/// Where Node and the Agent Tools entry point live on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolsInstall {
    pub node: PathBuf,
    pub entry: PathBuf,
}

impl AgentToolsInstall {
    /// Reads `FACTORY_AGENT_TOOLS_NODE` and `FACTORY_AGENT_TOOLS_ENTRY`. `None` when either is
    /// unset or points at a missing file: the host then reports the tool profile as unavailable,
    /// which is not an error for anything else.
    pub fn from_env() -> Option<Self> {
        let node = PathBuf::from(std::env::var_os("FACTORY_AGENT_TOOLS_NODE")?);
        let entry = PathBuf::from(std::env::var_os("FACTORY_AGENT_TOOLS_ENTRY")?);
        (node.is_file() && entry.is_file()).then_some(Self { node, entry })
    }
}

/// The only environment variable names `agent_tools_spec` ever sets (plus `SystemRoot` on
/// Windows, which Node needs to start).
pub const ENV_ALLOWLIST: [&str; 9] = [
    "PATH",
    "HOME",
    "XDG_STATE_HOME",
    "XDG_DATA_HOME",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "HOI4_GAME_ROOT",
    "HOI4_AGENT_STDIO_IDLE_EXIT_MINUTES",
    "NODE_ENV",
];

#[cfg(unix)]
const FIXED_PATH: &str = "/usr/local/bin:/usr/bin:/bin";
#[cfg(not(unix))]
const FIXED_PATH: &str = "C:\\Windows\\System32";

/// Creates `state_dir` and returns its canonical path. Agent Tools refuses a state root that
/// contains a symbolic link or junction (`SERVER_STATE_UNSAFE`), and macOS temp folders sit
/// behind one, so the host passes the result of this function to [`agent_tools_spec`].
pub fn prepare_state_dir(state_dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(state_dir)?;
    state_dir.canonicalize()
}

/// Builds the launch spec. `state_dir` must be outside the mod so the server's caches and
/// artifacts never land in mod source.
pub fn agent_tools_spec(
    install: &AgentToolsInstall,
    mod_root: &Path,
    state_dir: &Path,
    game_root: Option<&Path>,
) -> McpServerSpec {
    let under = |name: &str| state_dir.join(name).to_string_lossy().into_owned();
    let mut env = vec![
        ("PATH".to_owned(), FIXED_PATH.to_owned()),
        ("HOME".to_owned(), under("home")),
        ("XDG_STATE_HOME".to_owned(), under("state")),
        ("XDG_DATA_HOME".to_owned(), under("data")),
        ("XDG_CONFIG_HOME".to_owned(), under("config")),
        ("XDG_CACHE_HOME".to_owned(), under("cache")),
    ];
    if let Some(game) = game_root {
        env.push((
            "HOI4_GAME_ROOT".to_owned(),
            game.to_string_lossy().into_owned(),
        ));
    }
    env.push((
        "HOI4_AGENT_STDIO_IDLE_EXIT_MINUTES".to_owned(),
        "10".to_owned(),
    ));
    env.push(("NODE_ENV".to_owned(), "production".to_owned()));
    #[cfg(windows)]
    env.push(("SystemRoot".to_owned(), "C:\\Windows".to_owned()));

    let mut spec = McpServerSpec::new(&install.node, mod_root);
    spec.args = vec![install.entry.to_string_lossy().into_owned()];
    spec.env = env;
    spec.startup_timeout = Duration::from_secs(60);
    spec.default_call_timeout = Duration::from_secs(120);
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_env_is_exactly_the_allowlist() {
        let install = AgentToolsInstall {
            node: "/opt/node22/bin/node".into(),
            entry: "/opt/hoi4-agent-tools/dist/bin/stdio.js".into(),
        };
        let spec = agent_tools_spec(
            &install,
            Path::new("/work/mod"),
            Path::new("/work/state"),
            Some(Path::new("/game")),
        );
        let mut keys: Vec<&str> = spec.env.iter().map(|(k, _)| k.as_str()).collect();
        keys.sort_unstable();
        let mut want = ENV_ALLOWLIST.to_vec();
        want.sort_unstable();
        #[cfg(windows)]
        want.push("SystemRoot");
        #[cfg(windows)]
        want.sort_unstable();
        assert_eq!(keys, want);
        assert_eq!(spec.cwd, Path::new("/work/mod"));
        for (k, v) in &spec.env {
            if k.starts_with("XDG_") || k == "HOME" {
                assert!(
                    v.starts_with("/work/state") || v.starts_with("/work\\state"),
                    "{k}"
                );
            }
        }
        let without_game = agent_tools_spec(&install, Path::new("/m"), Path::new("/s"), None);
        assert!(!without_game.env.iter().any(|(k, _)| k == "HOI4_GAME_ROOT"));
    }
}
