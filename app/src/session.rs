use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionData {
    pub window_width: f64,
    pub window_height: f64,
    pub workspaces: Vec<WorkspaceSession>,
    pub active_workspace_idx: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceSession {
    pub name: String,
    pub cwd: String,
}

fn session_path() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    config_dir.join("rumux").join("session.json")
}

pub fn save_session(data: &SessionData) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    let json = serde_json::to_string_pretty(data)?;
    std::fs::write(&path, json).context("Failed to write session file")?;
    Ok(())
}

pub fn load_session() -> Result<Option<SessionData>> {
    let path = session_path();
    if !path.exists() {
        return Ok(None);
    }
    let json = std::fs::read_to_string(&path).context("Failed to read session file")?;
    let data: SessionData =
        serde_json::from_str(&json).context("Failed to parse session file")?;
    Ok(Some(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");

        let data = SessionData {
            window_width: 1200.0,
            window_height: 800.0,
            workspaces: vec![
                WorkspaceSession {
                    name: "Main".to_string(),
                    cwd: "/home/user".to_string(),
                },
                WorkspaceSession {
                    name: "Dev".to_string(),
                    cwd: "/home/user/project".to_string(),
                },
            ],
            active_workspace_idx: 0,
        };

        let json = serde_json::to_string_pretty(&data).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: SessionData =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.workspaces.len(), 2);
        assert_eq!(loaded.workspaces[0].name, "Main");
        assert_eq!(loaded.active_workspace_idx, 0);
    }
}
