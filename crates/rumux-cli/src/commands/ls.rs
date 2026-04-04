use anyhow::Result;
use console::style;

use rumux_core::config::find_repo_root;
use rumux_core::git_ops::list_worktrees;

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)?;
    let worktrees = list_worktrees(&repo_root)?;

    if worktrees.is_empty() {
        eprintln!(
            "{}",
            style("No worktrees found. Create one with: rumux new <branch>").yellow()
        );
        return Ok(());
    }

    // Calculate column widths
    let max_name = worktrees.iter().map(|w| w.name.len()).max().unwrap_or(0);
    let max_branch = worktrees.iter().map(|w| w.branch.len()).max().unwrap_or(0);

    eprintln!(
        "  {:<width_n$}  {:<width_b$}  {}",
        style("NAME").bold(),
        style("BRANCH").bold(),
        style("SHA").bold(),
        width_n = max_name,
        width_b = max_branch,
    );

    for wt in &worktrees {
        let sha_display = if wt.exists {
            style(&wt.short_sha).dim().to_string()
        } else {
            style("missing").red().to_string()
        };

        eprintln!(
            "  {:<width_n$}  {:<width_b$}  {}",
            style(&wt.name).green(),
            &wt.branch,
            sha_display,
            width_n = max_name,
            width_b = max_branch,
        );
    }

    Ok(())
}
