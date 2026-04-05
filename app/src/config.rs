use anyhow::Result;
use rumux_core::runtime::rumux_config_dir;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RumuxConfig {
    pub font_family: String,
    pub font_size: f32,
    pub scrollback: usize,
    pub sidebar_visible: bool,
    pub sidebar_width: f32,
    pub notification_command: Option<String>,
}

impl Default for RumuxConfig {
    fn default() -> Self {
        Self {
            font_family: "JetBrains Mono".to_string(),
            font_size: 14.0,
            scrollback: 10_000,
            sidebar_visible: true,
            sidebar_width: 200.0,
            notification_command: None,
        }
    }
}

impl RumuxConfig {
    pub fn load() -> Self {
        // Try rumux config first
        if let Some(path) = rumux_config_path() {
            if path.exists() {
                if let Ok(config) = load_toml(&path) {
                    return config;
                }
            }
        }

        // Fall back to Ghostty config
        let mut config = RumuxConfig::default();
        for path in ghostty_config_paths() {
            if path.exists() {
                if let Ok(ghostty) = load_ghostty(&path) {
                    if let Some(font) = ghostty.0 {
                        config.font_family = font;
                    }
                    if let Some(size) = ghostty.1 {
                        config.font_size = size;
                    }
                    break;
                }
            }
        }

        config
    }
}

fn rumux_config_path() -> Option<PathBuf> {
    Some(rumux_config_dir().join("config.toml"))
}

fn ghostty_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("ghostty").join("config"));
    }

    #[cfg(target_os = "macos")]
    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join("Library/Application Support/com.mitchellh.ghostty/config"));
    }

    paths
}

fn load_toml(path: &PathBuf) -> Result<RumuxConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: RumuxConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Parse Ghostty config (line-based key = value format).
/// Returns (font_family, font_size).
fn load_ghostty(path: &PathBuf) -> Result<(Option<String>, Option<f32>)> {
    let content = std::fs::read_to_string(path)?;
    let mut font_family = None;
    let mut font_size = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "font-family" | "font_family" => {
                    font_family = Some(value.to_string());
                }
                "font-size" | "font_size" => {
                    if let Ok(v) = value.parse::<f32>() {
                        font_size = Some(v);
                    }
                }
                _ => {}
            }
        }
    }

    Ok((font_family, font_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RumuxConfig::default();
        assert_eq!(config.font_family, "JetBrains Mono");
        assert_eq!(config.font_size, 14.0);
        assert_eq!(config.scrollback, 10_000);
    }

    #[test]
    fn test_toml_parse() {
        let toml_str = r#"
font_family = "Fira Code"
font_size = 16.0
scrollback = 50000
"#;
        let config: RumuxConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font_family, "Fira Code");
        assert_eq!(config.font_size, 16.0);
        assert_eq!(config.scrollback, 50000);
        // Defaults for unset fields
        assert!(config.sidebar_visible);
    }

    #[test]
    fn test_ghostty_parse() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "font-family = Cascadia Code\nfont-size = 13\n").unwrap();
        let (font, size) = load_ghostty(&tmp.path().to_path_buf()).unwrap();
        assert_eq!(font.unwrap(), "Cascadia Code");
        assert_eq!(size.unwrap(), 13.0);
    }
}
