use anyhow::{Context, Result};
use std::process::Command;

/// Check if the claude CLI is available in PATH.
pub fn check_claude_cli() -> Result<()> {
    match which("claude") {
        true => Ok(()),
        false => Err(rumux_core::errors::RumuxError::ClaudeNotFound.into()),
    }
}

fn which(program: &str) -> bool {
    Command::new("which")
        .arg(program)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Launch claude in the given directory.
pub fn launch_claude(dir: &std::path::Path, prompt: Option<&str>) -> Result<()> {
    check_claude_cli()?;

    let mut cmd = Command::new("claude");
    cmd.current_dir(dir);

    if let Some(p) = prompt {
        cmd.arg("-p").arg(p);
    }

    let status = cmd
        .status()
        .context("Failed to launch Claude Code")?;

    if !status.success() {
        eprintln!(
            "{}",
            console::style(format!(
                "Claude exited with status {}",
                status.code().unwrap_or(-1)
            ))
            .dim()
        );
    }

    Ok(())
}

/// Launch claude with --continue flag to resume last session.
pub fn launch_claude_continue(dir: &std::path::Path) -> Result<()> {
    check_claude_cli()?;

    let status = Command::new("claude")
        .arg("--continue")
        .current_dir(dir)
        .status()
        .context("Failed to launch Claude Code")?;

    if !status.success() {
        eprintln!(
            "{}",
            console::style(format!(
                "Claude exited with status {}",
                status.code().unwrap_or(-1)
            ))
            .dim()
        );
    }

    Ok(())
}
