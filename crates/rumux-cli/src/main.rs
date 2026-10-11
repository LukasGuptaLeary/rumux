#![warn(clippy::all)]

mod commands;
mod hook;
mod shell;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(
    name = "rumux",
    version,
    about = "Native terminal workspace control and git worktrees for coding agents"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new worktree and launch a coding agent
    New {
        /// Branch name for the new worktree
        branch: String,
        /// Pass a prompt directly to Claude Code (runs claude -p)
        #[arg(short, long, conflicts_with = "no_agent")]
        prompt: Option<String>,
        #[command(flatten)]
        agent: shell::AgentOptions,
    },
    /// Launch a coding agent in an existing worktree
    Start {
        /// Branch name of the worktree to resume
        branch: String,
        #[command(flatten)]
        agent: shell::AgentOptions,
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
    /// Ping the rumux desktop app via socket
    Ping {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List capabilities of the rumux desktop app
    Capabilities {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Identify the rumux desktop app
    Identify {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Send a notification to the rumux desktop app
    Notify {
        /// Notification title
        #[arg(long)]
        title: String,
        /// Notification body
        #[arg(long)]
        body: String,
        /// Workspace index (defaults to the active workspace)
        #[arg(long = "workspace")]
        workspace_index: Option<usize>,
        /// Terminal index within the workspace
        #[arg(long = "surface")]
        surface_index: Option<usize>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Send a raw JSON-RPC method to the rumux desktop app
    Rpc {
        /// Method name (e.g. "system.ping")
        method: String,
        /// JSON params (default: {})
        #[arg(default_value = "")]
        params: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run a stdio MCP server that controls a running rumux desktop app
    Mcp,
    /// Open an SSH workspace in the running desktop app
    Ssh(commands::ssh::SshOptions),
    /// Inspect or stop the profile's persistent terminal service
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
}

#[derive(Subcommand)]
enum SessionCommand {
    /// List service-owned terminal processes, including detached panes
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Stop this profile's service and all of its terminal processes
    Stop,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::New {
            ref branch,
            ref prompt,
            ref agent,
        } => commands::new::run(branch, prompt.as_deref(), agent),
        Commands::Start {
            ref branch,
            ref agent,
        } => commands::start::run(branch, agent),
        Commands::Cd { ref branch } => commands::cd::run(branch.as_deref()),
        Commands::Ls => commands::ls::run(),
        Commands::Merge { ref branch, squash } => commands::merge::run(branch.as_deref(), squash),
        Commands::Rm {
            ref branch,
            all,
            force,
        } => commands::rm::run(branch.as_deref(), all, force),
        Commands::Init { replace } => commands::init::run(replace),
        Commands::Update => commands::update::run(),
        Commands::Version => commands::version::run(),
        Commands::Completions { shell } => commands::completions::run::<Cli>(shell),
        Commands::Ping { json } => commands::socket_client::run_ping(json),
        Commands::Capabilities { json } => commands::socket_client::run_capabilities(json),
        Commands::Identify { json } => commands::socket_client::run_identify(json),
        Commands::Notify {
            ref title,
            ref body,
            workspace_index,
            surface_index,
            json,
        } => commands::socket_client::run_notify(title, body, workspace_index, surface_index, json),
        Commands::Rpc {
            ref method,
            ref params,
            json,
        } => commands::socket_client::run_raw(method, params, json),
        Commands::Mcp => commands::mcp::run(),
        Commands::Ssh(ref options) => commands::ssh::run(options),
        Commands::Session { ref command } => run_session(command),
    };

    if let Err(e) = result {
        eprintln!("{} {e:#}", console::style("error:").red().bold());
        std::process::exit(1);
    }
}

fn run_session(command: &SessionCommand) -> anyhow::Result<()> {
    let client = rumux_core::terminal_service::Client::discover()?;
    match command {
        SessionCommand::Status { json } => {
            let processes = client.call("list", serde_json::json!({}))?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&processes)?);
            } else if let Some(processes) = processes.as_array() {
                for process in processes {
                    println!(
                        "{}  pid={}  {}",
                        process["id"].as_str().unwrap_or("?"),
                        process["pid"],
                        if process["exited"] == true {
                            "exited"
                        } else {
                            "running"
                        }
                    );
                }
            }
        }
        SessionCommand::Stop => {
            client.call("stop", serde_json::json!({}))?;
            eprintln!("Stopped terminal service for this profile.");
        }
    }
    Ok(())
}
