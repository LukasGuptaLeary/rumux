use thiserror::Error;

#[derive(Error, Debug)]
pub enum RumuxError {
    #[error("Worktree '{0}' not found.")]
    WorktreeNotFound(String),

    #[error("Not inside a rumux worktree. Specify a branch name or run from within a worktree.")]
    NotInWorktree,

    #[error("Claude Code CLI not found. Install it: https://docs.anthropic.com/en/docs/claude-code")]
    ClaudeNotFound,

    #[error("Merge conflict detected. Resolve conflicts in the worktree, then commit manually.")]
    MergeConflict,
}
