use anyhow::{Context, Result};
use gpui_component::dock::DockAreaState;
use rumux_core::runtime::rumux_config_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SESSION_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionData {
    pub schema_version: u32,
    pub window_width: f64,
    pub window_height: f64,
    pub workspaces: Vec<WorkspaceSession>,
    pub active_workspace_idx: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceSession {
    pub name: String,
    pub cwd: String,
    pub dock_area: Option<DockAreaState>,
    pub next_terminal_index: Option<usize>,
}

impl SessionData {
    pub fn new(workspaces: Vec<WorkspaceSession>, active_workspace_idx: usize) -> Self {
        Self {
            schema_version: SESSION_SCHEMA_VERSION,
            window_width: 0.0,
            window_height: 0.0,
            workspaces,
            active_workspace_idx,
        }
    }
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            schema_version: SESSION_SCHEMA_VERSION,
            window_width: 0.0,
            window_height: 0.0,
            workspaces: Vec::new(),
            active_workspace_idx: 0,
        }
    }
}

fn session_path() -> PathBuf {
    rumux_config_dir().join("session.json")
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
    let data: SessionData = serde_json::from_str(&json).context("Failed to parse session file")?;
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
            schema_version: SESSION_SCHEMA_VERSION,
            window_width: 1200.0,
            window_height: 800.0,
            workspaces: vec![
                WorkspaceSession {
                    name: "Main".to_string(),
                    cwd: "/home/user".to_string(),
                    dock_area: None,
                    next_terminal_index: Some(1),
                },
                WorkspaceSession {
                    name: "Dev".to_string(),
                    cwd: "/home/user/project".to_string(),
                    dock_area: None,
                    next_terminal_index: Some(2),
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
        assert_eq!(loaded.schema_version, SESSION_SCHEMA_VERSION);
    }

    #[test]
    fn test_legacy_session_still_deserializes() {
        let legacy = r#"{
  "window_width": 1200.0,
  "window_height": 800.0,
  "workspaces": [
    { "name": "Main", "cwd": "/tmp/project" }
  ],
  "active_workspace_idx": 0
}"#;

        let loaded: SessionData = serde_json::from_str(legacy).unwrap();
        assert_eq!(loaded.workspaces.len(), 1);
        assert_eq!(loaded.workspaces[0].cwd, "/tmp/project");
        assert!(loaded.workspaces[0].dock_area.is_none());
    }
}
