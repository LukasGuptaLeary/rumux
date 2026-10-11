//! Typed OpenSSH launch parameters for remote terminal workspaces.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalForward {
    pub local_port: u16,
    pub remote_port: u16,
    #[serde(default = "loopback")]
    pub remote_host: String,
}
fn loopback() -> String {
    "127.0.0.1".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshConnection {
    /// A hostname, IP address or alias from the user's OpenSSH configuration.
    pub host: String,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub directory: Option<String>,
    #[serde(default)]
    pub identity_file: Option<PathBuf>,
    #[serde(default)]
    pub known_hosts_file: Option<PathBuf>,
    #[serde(default)]
    pub config_file: Option<PathBuf>,
    #[serde(default)]
    pub forwards: Vec<LocalForward>,
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "._-:[]%".contains(ch))
}

impl SshConnection {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            valid_host(&self.host),
            "Invalid SSH hostname or config alias"
        );
        if let Some(user) = &self.user {
            ensure!(
                !user.is_empty()
                    && user.len() <= 256
                    && !user.starts_with('-')
                    && user
                        .chars()
                        .all(|ch| ch.is_alphanumeric() || "._-".contains(ch)),
                "Invalid SSH username"
            );
        }
        ensure!(self.port != Some(0), "SSH port cannot be zero");
        if let Some(directory) = &self.directory {
            ensure!(
                directory.starts_with('/')
                    && directory.len() <= 16_384
                    && !directory.chars().any(char::is_control),
                "Remote directory must be an absolute POSIX path"
            );
        }
        ensure!(
            self.forwards.len() <= 16,
            "At most 16 local forwards are supported"
        );
        let mut ports = std::collections::BTreeSet::new();
        for forward in &self.forwards {
            ensure!(
                forward.local_port > 0 && forward.remote_port > 0,
                "Forwarded ports cannot be zero"
            );
            ensure!(
                valid_host(&forward.remote_host),
                "Invalid forwarded hostname"
            );
            ensure!(
                ports.insert(forward.local_port),
                "Local forward ports must be unique"
            );
        }
        for path in [
            &self.identity_file,
            &self.known_hosts_file,
            &self.config_file,
        ]
        .into_iter()
        .flatten()
        {
            ensure!(
                !path.as_os_str().is_empty() && !path.as_os_str().to_string_lossy().contains('\0'),
                "Invalid SSH file path"
            );
        }
        Ok(())
    }

    pub fn destination(&self) -> String {
        self.user
            .as_ref()
            .map_or_else(|| self.host.clone(), |user| format!("{user}@{}", self.host))
    }

    /// Return argv for the OpenSSH executable. Host authentication remains
    /// OpenSSH's responsibility; this never disables known-host verification.
    pub fn arguments(&self, surface_id: &str) -> Result<Vec<String>> {
        self.validate()?;
        ensure!(
            surface_id.len() == 36
                && surface_id
                    .chars()
                    .all(|ch| ch.is_ascii_hexdigit() || ch == '-'),
            "Invalid surface ID"
        );
        let mut args = vec!["-tt".into(), "-o".into(), "ExitOnForwardFailure=yes".into()];
        if let Some(path) = &self.config_file {
            args.extend(["-F".into(), path.to_string_lossy().into_owned()]);
        }
        if let Some(port) = self.port {
            args.extend(["-p".into(), port.to_string()]);
        }
        if let Some(path) = &self.identity_file {
            args.extend([
                "-i".into(),
                path.to_string_lossy().into_owned(),
                "-o".into(),
                "IdentitiesOnly=yes".into(),
            ]);
        }
        if let Some(path) = &self.known_hosts_file {
            // OpenSSH parses values in -o as configuration syntax. Quote paths
            // containing spaces, and reject config-line delimiters.
            let path = path.to_string_lossy();
            ensure!(
                !path.chars().any(char::is_control),
                "Invalid known-hosts file path"
            );
            let quoted = format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""));
            args.extend([
                "-o".into(),
                format!("UserKnownHostsFile={quoted}"),
                "-o".into(),
                "StrictHostKeyChecking=yes".into(),
            ]);
        }
        for forward in &self.forwards {
            let host = if forward.remote_host.contains(':') && !forward.remote_host.starts_with('[')
            {
                format!("[{}]", forward.remote_host)
            } else {
                forward.remote_host.clone()
            };
            args.extend([
                "-L".into(),
                format!(
                    "127.0.0.1:{}:{host}:{}",
                    forward.local_port, forward.remote_port
                ),
            ]);
        }
        args.extend(["--".into(), self.destination()]);
        let cd = self.directory.as_ref().map_or_else(String::new, |path| {
            format!("cd -- {} || exit; ", quote_posix(path))
        });
        args.push(format!("{cd}export TERM_PROGRAM=rumux RUMUX_SURFACE_ID={surface_id}; exec \"${{SHELL:-/bin/sh}}\" -l"));
        Ok(args)
    }

    /// New SSH tabs share the destination and directory. Local listeners belong
    /// to the pane that requested them and cannot be bound again by another pane.
    pub fn for_new_pane(&self, directory: Option<String>) -> Self {
        let mut connection = self.clone();
        connection.forwards.clear();
        connection.directory = directory.or_else(|| self.directory.clone());
        connection
    }
}

pub fn quote_posix(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn connection() -> SshConnection {
        serde_json::from_str(
            r#"{"host":"dev-box","user":"tester","directory":"/work/quoted ' directory"}"#,
        )
        .unwrap()
    }
    #[test]
    fn ssh_arguments_keep_options_and_remote_paths_separate() {
        let mut connection = connection();
        connection.forwards.push(LocalForward {
            local_port: 3001,
            remote_port: 3000,
            remote_host: "::1".into(),
        });
        let args = connection
            .arguments("23b91118-fdd1-4d9a-9890-e7f96e7c55ac")
            .unwrap();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["-L", "127.0.0.1:3001:[::1]:3000"])
        );
        assert!(args.windows(2).any(|pair| pair == ["--", "tester@dev-box"]));
        assert!(
            args.last()
                .unwrap()
                .contains("cd -- '/work/quoted '\"'\"' directory'")
        );
        assert!(
            !args
                .iter()
                .any(|arg| arg.contains("StrictHostKeyChecking=no"))
        );
    }
    #[test]
    fn ssh_rejects_option_injection_and_conflicting_forward_ports() {
        for host in ["-oProxyCommand=bad", "host;bad", "user@host", "host\nother"] {
            let mut connection = connection();
            connection.host = host.into();
            assert!(connection.validate().is_err());
        }
        let mut connection = connection();
        connection.forwards = vec![
            LocalForward {
                local_port: 3000,
                remote_port: 3000,
                remote_host: loopback()
            };
            2
        ];
        assert!(connection.validate().is_err());
    }
    #[test]
    fn ssh_new_panes_inherit_directory_and_do_not_rebind_listeners() {
        let mut connection = connection();
        connection.forwards.push(LocalForward {
            local_port: 3000,
            remote_port: 3000,
            remote_host: loopback(),
        });
        let next = connection.for_new_pane(Some("/different".into()));
        assert_eq!(next.host, connection.host);
        assert_eq!(next.directory.as_deref(), Some("/different"));
        assert!(next.forwards.is_empty());
    }
}
