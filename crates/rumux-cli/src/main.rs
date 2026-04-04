#![warn(clippy::all)]

mod commands;
mod hook;
mod shell;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(name = "rumux", version, about = "Git worktree lifecycle manager for parallel Claude Code sessions")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new worktree and launch Claude Code
    New {
        /// Branch name for the new worktree
        branch: String,
        /// Pass a prompt directly to Claude Code (runs claude -p)
        #[arg(short, long)]
        prompt: Option<String>,
    },
    /// Resume Claude Code in an existing worktree
    Start {
        /// Branch name of the worktree to resume
        branch: String,
    },
    /// Print worktree path (for use with shell cd wrapper)
    Cd {
        /// Branch name (omit to print repo root)
        branch: Option<String>,
    },
    /// List active worktrees
    Ls,
    /// Merge a worktree branch into the current branch
    Merge {
        /// Branch name to merge (auto-detected if inside a worktree)
        branch: Option<String>,
        /// Perform a squash merge
        #[arg(long)]
        squash: bool,
    },
    /// Remove a worktree and its branch
    Rm {
        /// Branch name to remove (auto-detected if inside a worktree)
        branch: Option<String>,
        /// Remove all worktrees
        #[arg(long)]
        all: bool,
        /// Force delete unmerged branches
        #[arg(long)]
        force: bool,
    },
    /// Generate a .rumux/setup hook using Claude
    Init {
        /// Overwrite existing setup hook
        #[arg(long)]
        replace: bool,
    },
    /// Show update instructions
    Update,
    /// Print version
    Version,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::New { ref branch, ref prompt } => {
            commands::new::run(branch, prompt.as_deref())
        }
        Commands::Start { ref branch } => commands::start::run(branch),
        Commands::Cd { ref branch } => commands::cd::run(branch.as_deref()),
        Commands::Ls => commands::ls::run(),
        Commands::Merge { ref branch, squash } => {
            commands::merge::run(branch.as_deref(), squash)
        }
        Commands::Rm { ref branch, all, force } => {
            commands::rm::run(branch.as_deref(), all, force)
        }
        Commands::Init { replace } => commands::init::run(replace),
        Commands::Update => commands::update::run(),
        Commands::Version => commands::version::run(),
        Commands::Completions { shell } => commands::completions::run::<Cli>(shell),
    };

    if let Err(e) = result {
        eprintln!("{} {e:#}", console::style("error:").red().bold());
        std::process::exit(1);
    }
}
