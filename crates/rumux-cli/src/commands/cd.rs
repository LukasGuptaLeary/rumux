use anyhow::Result;

use rumux_core::config::{find_repo_root, sanitize_branch_name, worktree_path};
use rumux_core::errors::RumuxError;

pub fn run(branch: Option<&str>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)?;

    match branch {
        None => {
            // Print repo root
            print!("{}", repo_root.display());
        }
        Some(b) => {
            let sanitized = sanitize_branch_name(b);
            let wt_path = worktree_path(&repo_root, &sanitized);
            if !wt_path.exists() {
                return Err(RumuxError::WorktreeNotFound(sanitized).into());
            }
            print!("{}", wt_path.display());
        }
    }

    Ok(())
}
