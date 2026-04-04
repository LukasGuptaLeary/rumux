use anyhow::{Context, Result};
use console::style;
use std::path::Path;
use std::process::Command;

/// Find the setup hook script. Checks .rumux/setup first, then .cmux/setup for backward compat.
/// Returns (path_to_hook, is_legacy) or None if no hook exists.
pub fn find_setup_hook(worktree_path: &Path) -> Option<(std::path::PathBuf, bool)> {
    let rumux_hook = worktree_path.join(".rumux").join("setup");
    if rumux_hook.exists() {
        return Some((rumux_hook, false));
    }

    let cmux_hook = worktree_path.join(".cmux").join("setup");
    if cmux_hook.exists() {
        return Some((cmux_hook, true));
    }

    None
}

/// Execute the setup hook in the given worktree directory.
/// Returns Ok(true) if the hook ran successfully, Ok(false) if no hook was found.
pub fn run_setup_hook(worktree_path: &Path) -> Result<bool> {
    let (hook_path, is_legacy) = match find_setup_hook(worktree_path) {
        Some(h) => h,
        None => return Ok(false),
    };

    if is_legacy {
        eprintln!(
            "{}",
            style("Using legacy .cmux/setup hook. Consider renaming to .rumux/setup.")
                .yellow()
        );
    }

    let spinner = console::Term::stderr();
    let _ = spinner;
    eprintln!(
        "{}",
        style("Running setup hook...").dim()
    );

    let status = Command::new(&hook_path)
        .current_dir(worktree_path)
        .status()
        .with_context(|| format!("Failed to execute setup hook: {}", hook_path.display()))?;

    if !status.success() {
        eprintln!(
            "{}",
            style(format!(
                "Warning: Setup hook exited with status {}",
                status.code().unwrap_or(-1)
            ))
            .yellow()
        );
    }

    Ok(true)
}
