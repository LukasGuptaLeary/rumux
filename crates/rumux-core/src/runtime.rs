use std::net::SocketAddr;
#[cfg(not(unix))]
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

#[cfg(not(unix))]
const DEFAULT_TCP_PORT: u16 = 62357;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcEndpoint {
    #[cfg(unix)]
    Unix(PathBuf),
    Tcp(SocketAddr),
}

pub fn rumux_config_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rumux")
}

pub fn default_shell() -> String {
    #[cfg(windows)]
    {
        std::env::var("RUMUX_SHELL")
            .or_else(|_| std::env::var("COMSPEC"))
            .unwrap_or_else(|_| "powershell.exe".to_string())
    }

    #[cfg(not(windows))]
    {
        std::env::var("RUMUX_SHELL")
            .or_else(|_| std::env::var("SHELL"))
            .unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

pub fn ipc_endpoint() -> IpcEndpoint {
    if let Some(addr) = std::env::var("RUMUX_SOCKET_ADDR")
        .ok()
        .and_then(|value| value.parse::<SocketAddr>().ok())
    {
        return IpcEndpoint::Tcp(addr);
    }

    #[cfg(unix)]
    if let Some(path) = std::env::var("RUMUX_SOCKET_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
    {
        return IpcEndpoint::Unix(path);
    }

    #[cfg(unix)]
    {
        IpcEndpoint::Unix(PathBuf::from("/tmp/rumux.sock"))
    }

    #[cfg(not(unix))]
    {
        IpcEndpoint::Tcp(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            DEFAULT_TCP_PORT,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_ends_with_rumux() {
        assert!(rumux_config_dir().ends_with("rumux"));
    }
}
