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

fn current_user_tag() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect()
        })
        .unwrap_or_else(|| "user".to_string())
}

fn endpoint_tag(endpoint: &IpcEndpoint) -> String {
    match endpoint {
        #[cfg(unix)]
        IpcEndpoint::Unix(path) => path
            .file_stem()
            .or_else(|| path.file_name())
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .map(|name| {
                name.chars()
                    .map(|ch| {
                        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                            ch
                        } else {
                            '_'
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(|| "socket".to_string()),
        IpcEndpoint::Tcp(addr) => format!("tcp-{}", addr.port()),
    }
}

pub fn rumux_config_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rumux")
}

pub fn rumux_runtime_dir() -> PathBuf {
    if let Some(path) = std::env::var("RUMUX_RUNTIME_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
    {
        return path;
    }

    dirs::runtime_dir()
        .map(|path| path.join("rumux"))
        .unwrap_or_else(|| std::env::temp_dir().join(format!("rumux-{}", current_user_tag())))
}

pub fn instance_lock_path() -> PathBuf {
    rumux_runtime_dir().join(format!("{}.lock", endpoint_tag(&ipc_endpoint())))
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
        IpcEndpoint::Unix(rumux_runtime_dir().join("rumux.sock"))
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

    #[test]
    fn runtime_dir_ends_with_rumux_or_user_scoped_fallback() {
        let runtime_dir = rumux_runtime_dir();
        assert!(
            runtime_dir.ends_with("rumux")
                || runtime_dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rumux-"))
        );
    }

    #[test]
    fn instance_lock_path_has_lock_suffix() {
        assert!(
            instance_lock_path()
                .extension()
                .is_some_and(|ext| ext == "lock")
        );
    }
}
