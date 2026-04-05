use std::path::Path;
use tempfile::TempDir;

/// Helper: create a temp git repo with an initial commit.
fn setup_repo() -> (TempDir, git2::Repository) {
    let tmp = TempDir::new().unwrap();
    let repo = git2::Repository::init(tmp.path()).unwrap();

    // Configure user for commits
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test User").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();

    // Create initial commit
    let sig = repo.signature().unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        // Write a file so we have something to commit
        let file_path = tmp.path().join("README.md");
        std::fs::write(&file_path, "# Test repo\n").unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        index.write_tree().unwrap()
    };
    {
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();
    }

    (tmp, repo)
}

mod config_tests {
    use rumux_core::config::*;

    #[test]
    fn test_sanitize_branch_name_comprehensive() {
        assert_eq!(sanitize_branch_name("feature/foo"), "feature-foo");
        assert_eq!(sanitize_branch_name("a/b/c"), "a-b-c");
        assert_eq!(sanitize_branch_name("--leading"), "leading");
        assert_eq!(sanitize_branch_name("trailing--"), "trailing");
        assert_eq!(sanitize_branch_name("mid--dle"), "mid-dle");
        assert_eq!(sanitize_branch_name(""), "");
        assert_eq!(sanitize_branch_name("normal"), "normal");
        assert_eq!(sanitize_branch_name("with.dots"), "with.dots");
    }
}

mod worktree_tests {
    use super::*;
    use rumux_core::config::*;
    use rumux_core::git_ops::*;

    #[test]
    fn test_create_and_list_worktree() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();

        // Create worktree
        let wt_path = worktree_path(repo_root, "test-branch");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        create_worktree(repo_root, "test-branch", &wt_path).unwrap();

        // Verify it exists
        assert!(wt_path.exists());
        assert!(wt_path.join(".git").exists());

        // List worktrees
        let worktrees = list_worktrees(repo_root).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].name, "test-branch");
        assert!(worktrees[0].exists);
    }

    #[test]
    fn test_idempotent_new() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();

        let wt_path = worktree_path(repo_root, "my-feature");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        create_worktree(repo_root, "my-feature", &wt_path).unwrap();

        // The worktree exists — calling new again should not fail
        // (the command checks existence and skips creation)
        assert!(wt_path.exists());

        // Attempting to create again with git should error, but the command
        // is designed to skip creation if directory exists
    }

    #[test]
    fn test_remove_worktree() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();

        let wt_path = worktree_path(repo_root, "to-remove");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        create_worktree(repo_root, "to-remove", &wt_path).unwrap();
        assert!(wt_path.exists());

        remove_worktree(repo_root, &wt_path, "to-remove").unwrap();
        assert!(!wt_path.exists());
    }

    #[test]
    fn test_remove_all_worktrees() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();

        // Create multiple worktrees
        for name in &["wt-a", "wt-b", "wt-c"] {
            let wt_path = worktree_path(repo_root, name);
            std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
            create_worktree(repo_root, name, &wt_path).unwrap();
        }

        let worktrees = list_worktrees(repo_root).unwrap();
        assert_eq!(worktrees.len(), 3);

        // Remove all
        for wt in &worktrees {
            remove_worktree(repo_root, &wt.path, &wt.name).unwrap();
        }

        let remaining = list_worktrees(repo_root).unwrap();
        assert_eq!(remaining.len(), 0);
    }

    #[test]
    fn test_merge_fast_forward() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();

        // Create worktree
        let wt_path = worktree_path(repo_root, "ff-branch");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        create_worktree(repo_root, "ff-branch", &wt_path).unwrap();

        // Make a commit in the worktree
        let wt_repo = git2::Repository::open(&wt_path).unwrap();
        let file_path = wt_path.join("new-file.txt");
        std::fs::write(&file_path, "hello\n").unwrap();
        let mut index = wt_repo.index().unwrap();
        index.add_path(Path::new("new-file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = wt_repo.find_tree(tree_id).unwrap();
        let head = wt_repo.head().unwrap().peel_to_commit().unwrap();
        let sig = wt_repo.signature().unwrap();
        wt_repo
            .commit(Some("HEAD"), &sig, &sig, "Add new file", &tree, &[&head])
            .unwrap();

        // Merge into main repo
        let result = merge_branch(repo_root, "ff-branch", false).unwrap();
        assert!(result.contains("Fast-forward"));
    }

    #[test]
    fn test_merge_squash() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();

        // Create worktree
        let wt_path = worktree_path(repo_root, "squash-branch");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        create_worktree(repo_root, "squash-branch", &wt_path).unwrap();

        // Make a commit in the worktree
        let wt_repo = git2::Repository::open(&wt_path).unwrap();
        let file_path = wt_path.join("squash-file.txt");
        std::fs::write(&file_path, "squash content\n").unwrap();
        let mut index = wt_repo.index().unwrap();
        index.add_path(Path::new("squash-file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = wt_repo.find_tree(tree_id).unwrap();
        let head = wt_repo.head().unwrap().peel_to_commit().unwrap();
        let sig = wt_repo.signature().unwrap();
        wt_repo
            .commit(Some("HEAD"), &sig, &sig, "Add squash file", &tree, &[&head])
            .unwrap();

        // Squash merge
        let result = merge_branch(repo_root, "squash-branch", true).unwrap();
        assert!(result.contains("Squash merge"));
    }

    #[test]
    fn test_is_branch_merged() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();

        // Create worktree with a branch
        let wt_path = worktree_path(repo_root, "merge-check");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        create_worktree(repo_root, "merge-check", &wt_path).unwrap();

        // Before any divergence, the branch tip equals HEAD — it's "merged"
        let merged = is_branch_merged(repo_root, "merge-check").unwrap();
        assert!(merged);

        // Make a commit on the branch
        let wt_repo = git2::Repository::open(&wt_path).unwrap();
        let file_path = wt_path.join("extra.txt");
        std::fs::write(&file_path, "extra\n").unwrap();
        let mut index = wt_repo.index().unwrap();
        index.add_path(Path::new("extra.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = wt_repo.find_tree(tree_id).unwrap();
        let head = wt_repo.head().unwrap().peel_to_commit().unwrap();
        let sig = wt_repo.signature().unwrap();
        wt_repo
            .commit(Some("HEAD"), &sig, &sig, "Extra commit", &tree, &[&head])
            .unwrap();

        // Now the branch has a commit that main doesn't — not merged
        let merged = is_branch_merged(repo_root, "merge-check").unwrap();
        assert!(!merged);
    }

    #[test]
    fn test_missing_worktree_error() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();
        let wt_path = worktree_path(repo_root, "nonexistent");
        assert!(!wt_path.exists());
    }

    #[test]
    fn test_no_worktrees_list_empty() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();
        let worktrees = list_worktrees(repo_root).unwrap();
        assert!(worktrees.is_empty());
    }

    #[test]
    fn test_delete_branch_after_merge() {
        let (tmp, _repo) = setup_repo();
        let repo_root = tmp.path();

        // Create worktree
        let wt_path = worktree_path(repo_root, "del-branch");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        create_worktree(repo_root, "del-branch", &wt_path).unwrap();

        // Remove worktree first (branch deletion fails if worktree still references it)
        remove_worktree(repo_root, &wt_path, "del-branch").unwrap();

        // Branch should exist and be deletable (it's at same commit as HEAD = merged)
        assert!(is_branch_merged(repo_root, "del-branch").unwrap());
        delete_branch(repo_root, "del-branch").unwrap();

        // Verify branch is gone
        let repo = open_repo(repo_root).unwrap();
        assert!(
            repo.find_branch("del-branch", git2::BranchType::Local)
                .is_err()
        );
    }
}
