use anyhow::Result;
use console::style;

use rumux_core::config::{find_repo_root, sanitize_branch_name, worktree_path};
use rumux_core::git_ops::create_worktree;
use crate::hook::run_setup_hook;
use crate::shell::launch_claude;

pub fn run(branch: &str, prompt: Option<&str>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)?;
    let sanitized = sanitize_branch_name(branch);

    if sanitized.is_empty() {
        anyhow::bail!("Invalid branch name: '{branch}'");
    }

    let wt_path = worktree_path(&repo_root, &sanitized);

    if wt_path.exists() {
        eprintln!(
            "{}",
            style("Worktree already exists, reusing...").yellow()
        );
    } else {
        eprintln!(
            "{}",
            style(format!("Creating worktree '{sanitized}'...")).dim()
        );

        std::fs::create_dir_all(wt_path.parent().unwrap_or(&repo_root))?;
        create_worktree(&repo_root, &sanitized, &wt_path)?;

        eprintln!("{}", style("Worktree created.").green());

        // Run setup hook
        run_setup_hook(&wt_path)?;
    }

    // Launch claude
    eprintln!(
        "{}",
        style(format!("Launching Claude Code in {}...", wt_path.display())).dim()
    );
    launch_claude(&wt_path, prompt)?;

    Ok(())
}
