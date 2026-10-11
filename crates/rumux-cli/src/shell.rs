use anyhow::{Context, Result};
use clap::Args;
use std::process::Command;

#[derive(Debug, Default, Args)]
pub struct AgentOptions {
    /// Agent executable (defaults to RUMUX_AGENT, then claude)
    #[arg(long)]
    pub agent: Option<String>,
    /// Argument passed directly to the agent; repeat for multiple arguments
    #[arg(long = "agent-arg", allow_hyphen_values = true)]
    pub agent_args: Vec<String>,
    /// Prepare or locate the worktree without launching an agent
    #[arg(long, conflicts_with_all = ["agent", "agent_args"])]
    pub no_agent: bool,
}

pub fn check_claude_cli() -> Result<()> {
    which::which("claude")
        .map(|_| ())
        .map_err(|_| rumux_core::errors::RumuxError::ClaudeNotFound.into())
}

pub fn launch_agent(
    dir: &std::path::Path,
    prompt: Option<&str>,
    resume: bool,
    options: &AgentOptions,
) -> Result<()> {
    let program = options
        .agent
        .clone()
        .or_else(|| {
            std::env::var("RUMUX_AGENT")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "claude".into());
    let executable = which::which(&program).with_context(|| {
        format!("Agent executable '{program}' was not found; use --agent or --no-agent")
    })?;
    let is_claude = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("claude"));
    let mut cmd = Command::new(executable);
    cmd.current_dir(dir).args(&options.agent_args);
    if is_claude && resume && options.agent_args.is_empty() {
        cmd.arg("--continue");
    }
    if let Some(prompt) = prompt {
        if is_claude {
            cmd.arg("-p");
        }
        cmd.arg(prompt);
    }
    let status = cmd
        .status()
        .with_context(|| format!("Failed to launch agent '{program}'"))?;
    anyhow::ensure!(status.success(), "Agent '{program}' exited with {status}");
    Ok(())
}
