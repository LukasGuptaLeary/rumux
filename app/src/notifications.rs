use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub workspace_id: usize,
    pub title: String,
    pub subtitle: Option<String>,
    pub body: String,
    pub timestamp: u64,
    pub read: bool,
}

/// Parse OSC notification escape sequences from terminal output.
///
/// Supported formats:
/// - OSC 9 ; <message> ST  (iTerm2/ConEmu)
/// - OSC 99 ; <params> ST  (Kitty)
/// - OSC 777 ; notify ; <title> ; <body> ST  (RXVT/URxvt)
#[allow(dead_code)]
pub fn parse_notifications(data: &str) -> Vec<(String, Option<String>, String)> {
    let mut results = Vec::new();
    let mut pos = 0;
    let bytes = data.as_bytes();

    while pos < bytes.len() {
        if bytes[pos] == 0x1b && pos + 1 < bytes.len() && bytes[pos + 1] == b']' {
            pos += 2;
            let start = pos;
            while pos < bytes.len() {
                if bytes[pos] == 0x07 {
                    let content = &data[start..pos];
                    if let Some(notif) = parse_osc_content(content) {
                        results.push(notif);
                    }
                    pos += 1;
                    break;
                }
                if bytes[pos] == 0x1b && pos + 1 < bytes.len() && bytes[pos + 1] == b'\\' {
                    let content = &data[start..pos];
                    if let Some(notif) = parse_osc_content(content) {
                        results.push(notif);
                    }
                    pos += 2;
                    break;
                }
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }

    results
}

#[allow(dead_code)]
fn parse_osc_content(content: &str) -> Option<(String, Option<String>, String)> {
    if let Some(rest) = content.strip_prefix("777;notify;") {
        let parts: Vec<&str> = rest.splitn(2, ';').collect();
        let title = parts.first().unwrap_or(&"").to_string();
        let body = parts.get(1).unwrap_or(&"").to_string();
        return Some((title, None, body));
    }

    if let Some(rest) = content.strip_prefix("9;") {
        return Some(("Notification".to_string(), None, rest.to_string()));
    }

    if let Some(rest) = content.strip_prefix("99;") {
        return Some(("Notification".to_string(), None, rest.to_string()));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_osc_777() {
        let data = "\x1b]777;notify;Test Title;Test Body\x07";
        let result = parse_notifications(data);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "Test Title");
        assert_eq!(result[0].2, "Test Body");
    }

    #[test]
    fn test_parse_osc_9() {
        let data = "\x1b]9;Hello World\x07";
        let result = parse_notifications(data);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].2, "Hello World");
    }

    #[test]
    fn test_parse_no_notification() {
        let data = "regular terminal output";
        let result = parse_notifications(data);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_osc_777_with_st() {
        let data = "\x1b]777;notify;Title;Body\x1b\\";
        let result = parse_notifications(data);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "Title");
    }
}
