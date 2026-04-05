use anyhow::Result;
use rumux_core::runtime::rumux_config_dir;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct CustomCommandDef {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CommandFile {
    #[serde(default)]
    commands: Vec<CustomCommandDef>,
}

pub fn load_custom_commands() -> Vec<CustomCommandDef> {
    let mut commands = Vec::new();

    // Load from CWD
    for filename in &["rumux.json", "cmux.json"] {
        let path = PathBuf::from(filename);
        if let Ok(cmds) = load_from_file(&path) {
            commands.extend(cmds);
        }
    }

    // Load from config dir
    if let Some(config_dir) = config_commands_dir() {
        for filename in &["commands.json", "rumux.json", "cmux.json"] {
            let path = config_dir.join(filename);
            if let Ok(cmds) = load_from_file(&path) {
                commands.extend(cmds);
            }
        }
    }

    commands
}

fn config_commands_dir() -> Option<PathBuf> {
    Some(rumux_config_dir())
}

fn load_from_file(path: &PathBuf) -> Result<Vec<CustomCommandDef>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(path)?;

    // Try parsing as { "commands": [...] } first
    if let Ok(file) = serde_json::from_str::<CommandFile>(&content) {
        return Ok(file.commands);
    }

    // Try parsing as a plain array [...]
    if let Ok(cmds) = serde_json::from_str::<Vec<CustomCommandDef>>(&content) {
        return Ok(cmds);
    }

    Ok(vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_commands_object() {
        let json = r#"{"commands": [{"name": "Test", "command": "echo test\n"}]}"#;
        let file: CommandFile = serde_json::from_str(json).unwrap();
        assert_eq!(file.commands.len(), 1);
        assert_eq!(file.commands[0].name, "Test");
        assert_eq!(file.commands[0].command, "echo test\n");
    }

    #[test]
    fn test_parse_commands_array() {
        let json =
            r#"[{"name": "Build", "command": "cargo build\n", "keywords": ["rust", "compile"]}]"#;
        let cmds: Vec<CustomCommandDef> = serde_json::from_str(json).unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].keywords, vec!["rust", "compile"]);
    }
}
