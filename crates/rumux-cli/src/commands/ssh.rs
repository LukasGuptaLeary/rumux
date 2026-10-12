use anyhow::{Result, ensure};
use clap::Args;
use rumux_core::rpc::send_rpc;
use rumux_core::ssh::{LocalForward, SshConnection};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct SshOptions {
    /// OpenSSH host/config alias, optionally USER@HOST
    pub destination: String,
    #[arg(short, long)]
    pub user: Option<String>,
    #[arg(short, long)]
    pub port: Option<u16>,
    /// Absolute directory on the POSIX remote host
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(short = 'i', long)]
    pub identity_file: Option<PathBuf>,
    /// Use this known-hosts file with strict host-key checking
    #[arg(long)]
    pub known_hosts_file: Option<PathBuf>,
    #[arg(short = 'F', long)]
    pub config_file: Option<PathBuf>,
    /// Forward LOCAL_PORT:REMOTE_PORT between loopback interfaces (repeatable)
    #[arg(short = 'L', long = "forward", value_parser = parse_forward)]
    pub forwards: Vec<LocalForward>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub json: bool,
}

fn parse_forward(value: &str) -> Result<LocalForward, String> {
    let (local, remote) = value
        .split_once(':')
        .ok_or_else(|| "Use LOCAL_PORT:REMOTE_PORT".to_string())?;
    let port = |value: &str| -> Result<u16, String> {
        value
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
            .ok_or_else(|| "Forward ports must be between 1 and 65535".to_string())
    };
    Ok(LocalForward {
        local_port: port(local)?,
        remote_port: port(remote)?,
        remote_host: "127.0.0.1".into(),
    })
}

impl SshOptions {
    fn connection(&self) -> Result<SshConnection> {
        let (user, host) = match self.destination.split_once('@') {
            Some((user, host)) => {
                ensure!(
                    self.user.is_none(),
                    "Specify the SSH user once, using USER@HOST or --user"
                );
                (Some(user.to_owned()), host.to_owned())
            }
            None => (self.user.clone(), self.destination.clone()),
        };
        let connection = SshConnection {
            host,
            user,
            port: self.port,
            directory: self.cwd.clone(),
            identity_file: self.identity_file.clone(),
            known_hosts_file: self.known_hosts_file.clone(),
            config_file: self.config_file.clone(),
            forwards: self.forwards.clone(),
        };
        connection.validate()?;
        Ok(connection)
    }
}

pub fn run(options: &SshOptions) -> Result<()> {
    let connection = options.connection()?;
    let response = send_rpc(
        "workspace.create",
        serde_json::json!({
            "name": options.name, "ssh": connection,
        }),
    )?;
    super::socket_client::ensure_success(&response)?;
    if options.json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        let id = response["result"]["id"].as_str().unwrap_or("unknown");
        println!("SSH workspace {id}: {}", connection.destination());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[derive(Parser)]
    struct TestArgs {
        #[command(flatten)]
        ssh: SshOptions,
    }

    #[test]
    fn parses_destination_and_repeatable_forwards_without_shell_expansion() {
        let options = TestArgs::try_parse_from([
            "ssh",
            "tester@dev-box",
            "--cwd",
            "/work/quoted ' directory",
            "-L",
            "3001:3000",
            "--forward",
            "8001:8000",
        ])
        .unwrap()
        .ssh;
        let connection = options.connection().unwrap();
        assert_eq!(connection.destination(), "tester@dev-box");
        assert_eq!(
            connection.directory.as_deref(),
            Some("/work/quoted ' directory")
        );
        assert_eq!(connection.forwards.len(), 2);
        assert_eq!(connection.forwards[1].remote_port, 8000);
    }

    #[test]
    fn rejects_invalid_ports_and_ambiguous_users_before_connecting() {
        for forward in ["0:80", "1:0", "65536:80", "1:host:80", "80"] {
            assert!(TestArgs::try_parse_from(["ssh", "host", "-L", forward]).is_err());
        }
        for args in [
            vec!["ssh", "user@host", "--user", "other"],
            vec!["ssh", "host;command"],
        ] {
            let options = TestArgs::try_parse_from(args).unwrap().ssh;
            assert!(options.connection().is_err());
        }
    }
}
