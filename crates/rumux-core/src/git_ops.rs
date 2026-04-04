use anyhow::{bail, Context, Result};
use git2::{BranchType, MergeAnalysis, Repository};
use std::path::Path;

use crate::errors::RumuxError;

/// Open the main repository from the repo root.
pub fn open_repo(repo_root: &Path) -> Result<Repository> {
    Repository::open(repo_root).context("Failed to open git repository")
}

/// Create a worktree with a new branch based on HEAD.
/// If the branch already exists, create the worktree using the existing branch.
pub fn create_worktree(repo_root: &Path, branch_name: &str, worktree_path: &Path) -> Result<()> {
    let repo = open_repo(repo_root)?;

    // Check if branch already exists
    let branch_exists = repo.find_branch(branch_name, BranchType::Local).is_ok();

    if branch_exists {
        // Use existing branch
        let branch_ref = format!("refs/heads/{branch_name}");
        let reference = repo
            .find_reference(&branch_ref)
            .context("Failed to find branch reference")?;
        let commit = reference.peel_to_commit().context("Failed to peel to commit")?;

        repo.worktree(
            branch_name,
            worktree_path,
            Some(git2::WorktreeAddOptions::new().reference(Some(&reference))),
        )
        .with_context(|| format!("Failed to create worktree for existing branch '{branch_name}'"))?;

        // The worktree creates a detached HEAD, we need to set it to the branch
        let wt_repo = Repository::open(worktree_path)?;
        wt_repo.set_head(&branch_ref)?;
        wt_repo.checkout_head(Some(
            git2::build::CheckoutBuilder::new().force(),
        ))?;

        let _ = commit; // used indirectly above
    } else {
        // Create new branch from HEAD
        let head = repo.head().context("Failed to get HEAD")?;
        let head_commit = head.peel_to_commit().context("HEAD does not point to a commit")?;

        // Create the branch first
        repo.branch(branch_name, &head_commit, false)
            .with_context(|| format!("Failed to create branch '{branch_name}'"))?;

        let branch_ref = format!("refs/heads/{branch_name}");
        let reference = repo.find_reference(&branch_ref)?;

        repo.worktree(
            branch_name,
            worktree_path,
            Some(git2::WorktreeAddOptions::new().reference(Some(&reference))),
        )
        .with_context(|| format!("Failed to create worktree at '{}'", worktree_path.display()))?;

        // Set HEAD to the branch in the worktree
        let wt_repo = Repository::open(worktree_path)?;
        wt_repo.set_head(&branch_ref)?;
        wt_repo.checkout_head(Some(
            git2::build::CheckoutBuilder::new().force(),
        ))?;
    }

    Ok(())
}

/// List worktree directories under .worktrees/ with their branch and HEAD SHA.
pub struct WorktreeInfo {
    pub name: String,
    pub branch: String,
    pub short_sha: String,
    pub path: std::path::PathBuf,
    pub exists: bool,
}

pub fn list_worktrees(repo_root: &Path) -> Result<Vec<WorktreeInfo>> {
    let worktrees_dir = repo_root.join(".worktrees");
    if !worktrees_dir.exists() {
        return Ok(vec![]);
    }

    let mut results = vec![];
    let entries = std::fs::read_dir(&worktrees_dir)
        .context("Failed to read .worktrees directory")?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let exists = path.join(".git").exists();

        let (branch, short_sha) = if exists {
            match Repository::open(&path) {
                Ok(wt_repo) => {
                    let branch = wt_repo
                        .head()
                        .ok()
                        .and_then(|h| h.shorthand().map(|s| s.to_string()))
                        .unwrap_or_else(|| "detached".to_string());
                    let sha = wt_repo
                        .head()
                        .ok()
                        .and_then(|h| h.peel_to_commit().ok())
                        .map(|c| c.id().to_string()[..7].to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    (branch, sha)
                }
                Err(_) => (name.clone(), "?".to_string()),
            }
        } else {
            (name.clone(), "missing".to_string())
        };

        results.push(WorktreeInfo {
            name,
            branch,
            short_sha,
            path,
            exists,
        });
    }

    results.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(results)
}

/// Check if a branch has been merged into the given target branch.
pub fn is_branch_merged(repo_root: &Path, branch_name: &str) -> Result<bool> {
    let repo = open_repo(repo_root)?;

    let head = repo.head().context("Failed to get HEAD")?;
    let head_commit = head.peel_to_commit()?;

    let branch = match repo.find_branch(branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(_) => return Ok(true), // Branch doesn't exist, consider it "merged"
    };

    let branch_commit = branch
        .get()
        .peel_to_commit()
        .context("Failed to get branch commit")?;

    // A branch is merged if its tip is an ancestor of (or equal to) HEAD
    if head_commit.id() == branch_commit.id() {
        return Ok(true);
    }
    let is_ancestor = repo.graph_descendant_of(head_commit.id(), branch_commit.id())?;
    Ok(is_ancestor)
}

/// Delete a local branch.
pub fn delete_branch(repo_root: &Path, branch_name: &str) -> Result<()> {
    let repo = open_repo(repo_root)?;
    let mut branch = repo
        .find_branch(branch_name, BranchType::Local)
        .with_context(|| format!("Branch '{branch_name}' not found"))?;
    branch
        .delete()
        .with_context(|| format!("Failed to delete branch '{branch_name}'"))?;
    Ok(())
}

/// Prune stale worktree references.
pub fn prune_worktrees(repo_root: &Path) -> Result<()> {
    let repo = open_repo(repo_root)?;
    // git2 doesn't have a direct prune API, so we clean up manually
    // by iterating worktrees and removing stale entries
    let worktree_names: Vec<String> = repo
        .worktrees()
        .context("Failed to list worktrees")?
        .iter()
        .flatten()
        .map(|s| s.to_string())
        .collect();

    for name in worktree_names {
        if let Ok(wt) = repo.find_worktree(&name)
            && wt.validate().is_err()
        {
            let mut opts = git2::WorktreePruneOptions::new();
            opts.valid(false).working_tree(false);
            wt.prune(Some(&mut opts)).ok();
        }
    }
    Ok(())
}

/// Perform a merge of the given branch into the primary checkout's current branch.
/// Returns a description of what happened.
pub fn merge_branch(repo_root: &Path, branch_name: &str, squash: bool) -> Result<String> {
    let repo = open_repo(repo_root)?;

    let branch = repo
        .find_branch(branch_name, BranchType::Local)
        .with_context(|| format!("Branch '{branch_name}' not found"))?;

    let branch_commit = branch
        .get()
        .peel_to_commit()
        .context("Failed to resolve branch to commit")?;

    let annotated_commit = repo
        .find_annotated_commit(branch_commit.id())
        .context("Failed to create annotated commit")?;

    let (analysis, _) = repo.merge_analysis(&[&annotated_commit])?;

    if analysis.contains(MergeAnalysis::ANALYSIS_UP_TO_DATE) {
        return Ok("Already up to date.".to_string());
    }

    if squash {
        // Squash merge: apply changes to index but don't commit
        repo.merge(&[&annotated_commit], None, None)
            .context("Merge failed")?;

        // Check for conflicts
        let index = repo.index()?;
        if index.has_conflicts() {
            bail!(RumuxError::MergeConflict);
        }

        // Clean up merge state
        repo.cleanup_state()?;

        return Ok(format!(
            "Squash merge of '{branch_name}' staged. Review and commit the changes manually."
        ));
    }

    if analysis.contains(MergeAnalysis::ANALYSIS_FASTFORWARD) {
        // Fast-forward
        let refname = format!("refs/heads/{}", repo.head()?.shorthand().unwrap_or("HEAD"));
        repo.find_reference(&refname)?.set_target(
            branch_commit.id(),
            &format!("Fast-forward merge of '{branch_name}'"),
        )?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;

        return Ok(format!(
            "Fast-forward merge of '{branch_name}' ({})",
            &branch_commit.id().to_string()[..7]
        ));
    }

    if analysis.contains(MergeAnalysis::ANALYSIS_NORMAL) {
        // Normal merge
        repo.merge(&[&annotated_commit], None, None)
            .context("Merge failed")?;

        let index = repo.index()?;
        if index.has_conflicts() {
            bail!(RumuxError::MergeConflict);
        }

        // Create merge commit
        let mut index = repo.index()?;
        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;
        let head_commit = repo.head()?.peel_to_commit()?;
        let sig = match repo.signature() {
            Ok(s) => s,
            Err(_) => git2::Signature::now("rumux", "rumux@localhost")?,
        };
        let message = format!("Merge branch '{branch_name}'");

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &message,
            &tree,
            &[&head_commit, &branch_commit],
        )?;

        repo.cleanup_state()?;

        return Ok(format!("Merge commit created for '{branch_name}'."));
    }

    bail!("Merge cannot be performed (analysis: {analysis:?})");
}

/// Remove a worktree directory and prune git worktree references.
pub fn remove_worktree(repo_root: &Path, worktree_path: &Path, name: &str) -> Result<()> {
    // Remove the directory
    if worktree_path.exists() {
        std::fs::remove_dir_all(worktree_path)
            .with_context(|| format!("Failed to remove worktree directory: {}", worktree_path.display()))?;
    }

    // Prune the worktree reference in git
    let repo = open_repo(repo_root)?;
    if let Ok(wt) = repo.find_worktree(name) {
        let mut opts = git2::WorktreePruneOptions::new();
        opts.valid(true).working_tree(true).locked(false);
        wt.prune(Some(&mut opts)).ok();
    }

    prune_worktrees(repo_root)?;
    Ok(())
}
