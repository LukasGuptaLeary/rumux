use anyhow::Result;
use console::style;

use crate::shell::{AgentOptions, launch_agent};
use rumux_core::config::{find_repo_root, sanitize_branch_name, worktree_path};
use rumux_core::errors::RumuxError;
use rumux_core::git_ops::list_worktrees;

pub fn run(branch: &str, agent: &AgentOptions) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)?;
    let sanitized = sanitize_branch_name(branch);
    let wt_path = worktree_path(&repo_root, &sanitized);

    if !wt_path.exists() {
        eprintln!(
            "{}",
            style(format!("Worktree '{sanitized}' not found.")).red()
        );

        let worktrees = list_worktrees(&repo_root)?;
        if !worktrees.is_empty() {
            eprintln!("\nAvailable worktrees:");
            for wt in &worktrees {
                eprintln!("  {}", style(&wt.name).green());
            }
        }

        return Err(RumuxError::WorktreeNotFound(sanitized).into());
    }

    if agent.no_agent {
        println!("{}", wt_path.display());
        return Ok(());
    }
    eprintln!(
        "{}",
        style(format!("Launching agent in {}...", wt_path.display())).dim()
    );
    launch_agent(&wt_path, None, true, agent)?;

    Ok(())
}
