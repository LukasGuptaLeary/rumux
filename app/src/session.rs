use anyhow::{Context, Result};
use gpui_component::dock::DockAreaState;
use rumux_core::runtime::rumux_config_dir;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

const SESSION_SCHEMA_VERSION: u32 = 3;

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
    pub id: Option<String>,
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
    save_session_to(&session_path(), data)
}

fn save_session_to(path: &Path, data: &SessionData) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    if let Ok(previous) = std::fs::read(path) {
        if decode_session(&previous).is_ok() {
            atomic_write(&path.with_extension("backup.json"), &previous)?;
        }
    }
    let json = serde_json::to_vec_pretty(data)?;
    atomic_write(path, &json)?;
    Ok(())
}

pub fn load_session() -> Result<Option<SessionData>> {
    load_session_from(&session_path())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Session path has no parent")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path)
        .context("Failed to atomically replace session file")?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn decode_session(bytes: &[u8]) -> Result<SessionData> {
    let mut data: SessionData =
        serde_json::from_slice(bytes).context("Failed to parse session file")?;
    anyhow::ensure!(
        data.schema_version <= SESSION_SCHEMA_VERSION,
        "Session schema {} is newer than supported schema {}; preserved without modification",
        data.schema_version,
        SESSION_SCHEMA_VERSION
    );
    data.schema_version = SESSION_SCHEMA_VERSION;
    Ok(data)
}

fn load_session_from(path: &Path) -> Result<Option<SessionData>> {
    if !path.exists() {
        return Ok(None);
    }
    let json = std::fs::read(path).context("Failed to read session file")?;
    match decode_session(&json) {
        Ok(data) => Ok(Some(data)),
        Err(error) => {
            // Never roll a future schema back to an older backup.
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&json) {
                if value
                    .get("schema_version")
                    .and_then(|v| v.as_u64())
                    .is_some_and(|version| version > SESSION_SCHEMA_VERSION as u64)
                {
                    return Err(error);
                }
            }
            let backup =
                std::fs::read(path.with_extension("backup.json")).context(error.to_string())?;
            let data = decode_session(&backup)?;
            let preserved = path.with_extension(format!("invalid-{}.json", uuid::Uuid::new_v4()));
            std::fs::copy(path, &preserved).context("Failed to preserve damaged session")?;
            eprintln!(
                "Recovered session from backup; damaged file preserved at {}",
                preserved.display()
            );
            Ok(Some(data))
        }
    }
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
                    id: None,
                    name: "Main".to_string(),
                    cwd: "/home/user".to_string(),
                    dock_area: None,
                    next_terminal_index: Some(1),
                },
                WorkspaceSession {
                    id: None,
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

    #[test]
    fn atomic_saves_recover_a_damaged_primary_and_preserve_evidence() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");
        let first = SessionData::new(
            vec![WorkspaceSession {
                name: "first".into(),
                ..Default::default()
            }],
            0,
        );
        let second = SessionData::new(
            vec![WorkspaceSession {
                name: "second".into(),
                ..Default::default()
            }],
            0,
        );
        save_session_to(&path, &first).unwrap();
        save_session_to(&path, &second).unwrap();
        assert_eq!(
            load_session_from(&path).unwrap().unwrap().workspaces[0].name,
            "second"
        );
        std::fs::write(&path, b"{partial").unwrap();
        assert_eq!(
            load_session_from(&path).unwrap().unwrap().workspaces[0].name,
            "first"
        );
        let invalid: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|result| result.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains("invalid-"))
            .collect();
        assert_eq!(invalid.len(), 1);
        assert_eq!(std::fs::read(invalid[0].path()).unwrap(), b"{partial");
        save_session_to(&path, &first).unwrap();
        assert_eq!(
            load_session_from(&path).unwrap().unwrap().workspaces[0].name,
            "first"
        );
    }

    #[test]
    fn future_schema_is_not_rolled_back_to_an_older_backup() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");
        save_session_to(&path, &SessionData::default()).unwrap();
        save_session_to(&path, &SessionData::default()).unwrap();
        let future = b"{\"schema_version\":999,\"workspaces\":[]}";
        std::fs::write(&path, future).unwrap();
        assert!(
            load_session_from(&path)
                .unwrap_err()
                .to_string()
                .contains("newer")
        );
        assert_eq!(std::fs::read(&path).unwrap(), future);
    }

    #[test]
    fn damaged_state_without_backup_reports_failure() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");
        std::fs::write(&path, b"not-json").unwrap();
        assert!(load_session_from(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"not-json");
    }
}
