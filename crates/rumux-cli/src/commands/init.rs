use anyhow::{Context, Result};
use console::style;

use rumux_core::config::find_repo_root;
use crate::shell::check_claude_cli;

pub fn run(replace: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)?;

    let rumux_dir = repo_root.join(".rumux");
    let setup_path = rumux_dir.join("setup");

    // Check for legacy .cmux/setup
    let cmux_setup = repo_root.join(".cmux").join("setup");
    if cmux_setup.exists() {
        eprintln!(
            "{}",
            style("Note: Found legacy .cmux/setup hook.").yellow()
        );
    }

    if setup_path.exists() && !replace {
        eprintln!(
            "{}",
            style("Setup hook already exists. Use --replace to overwrite.").yellow()
        );
        return Ok(());
    }

    check_claude_cli()?;

    eprintln!("{}", style("Generating setup hook with Claude...").dim());

    let output = std::process::Command::new("claude")
        .arg("-p")
        .arg(concat!(
            "Analyze this repository and generate a .rumux/setup bash script that handles ",
            "worktree initialization: installing dependencies, symlinking secrets/config files ",
            "from the main repo root, running codegen, etc. Output ONLY the bash script content ",
            "with a #!/bin/bash shebang. The script should use ",
            "REPO_ROOT=\"$(git rev-parse --git-common-dir | xargs dirname)\" to reference the main repo."
        ))
        .current_dir(&repo_root)
        .output()
        .context("Failed to run Claude CLI")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Claude CLI failed: {stderr}");
    }

    let script_content = String::from_utf8_lossy(&output.stdout);

    std::fs::create_dir_all(&rumux_dir)?;
    std::fs::write(&setup_path, script_content.as_bytes())?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&setup_path, std::fs::Permissions::from_mode(0o755))?;
    }

    eprintln!(
        "{}",
        style("Created .rumux/setup — review it, then commit to your repo.").green()
    );

    Ok(())
}
