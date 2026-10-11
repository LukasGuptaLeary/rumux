//! Bounded OSC parsing across arbitrary PTY read boundaries.
use base64::Engine;
use std::collections::BTreeMap;

const MAX_OSC_BYTES: usize = 16_384;
const MAX_PENDING_NOTIFICATIONS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalSignal {
    Notification { title: String, body: String },
    Title(String),
    WorkingDirectory(String),
}

#[derive(Default)]
pub struct TerminalProtocolParser {
    state: u8,
    payload: Vec<u8>,
    kitty: BTreeMap<String, (String, String)>,
}

impl TerminalProtocolParser {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<TerminalSignal> {
        let mut signals = Vec::new();
        for &byte in bytes {
            match self.state {
                0 => {
                    if byte == 0x1b {
                        self.state = 1;
                    }
                }
                1 => {
                    self.state = match byte {
                        b']' => 2,
                        0x1b => 1,
                        _ => 0,
                    };
                    self.payload.clear();
                }
                2 => match byte {
                    0x07 => self.finish(&mut signals),
                    0x1b => self.state = 3,
                    0x18 | 0x1a => {
                        self.payload.clear();
                        self.state = 0;
                    }
                    _ if self.payload.len() >= MAX_OSC_BYTES => {
                        self.payload.clear();
                        self.state = 4;
                    }
                    _ => self.payload.push(byte),
                },
                3 => match byte {
                    b'\\' => self.finish(&mut signals),
                    b']' => {
                        self.payload.clear();
                        self.state = 2;
                    }
                    0x1b => {
                        self.payload.clear();
                        self.state = 1;
                    }
                    _ => {
                        self.payload.clear();
                        self.state = 0;
                    }
                },
                // Discard oversized sequences through their terminator. A new
                // OSC can restart parsing even after a malformed producer.
                4 => match byte {
                    0x07 | 0x18 | 0x1a => self.state = 0,
                    0x1b => self.state = 5,
                    _ => {}
                },
                5 => {
                    self.state = match byte {
                        b']' => 2,
                        0x1b => 5,
                        b'\\' | 0x07 => 0,
                        _ => 4,
                    };
                }
                _ => self.state = 0,
            }
        }
        signals
    }

    fn finish(&mut self, signals: &mut Vec<TerminalSignal>) {
        self.state = 0;
        let bytes = std::mem::take(&mut self.payload);
        let content = String::from_utf8_lossy(&bytes);
        if let Some(signal) = self.parse(&content) {
            signals.push(signal);
        }
    }

    fn parse(&mut self, content: &str) -> Option<TerminalSignal> {
        let (code, payload) = content.split_once(';')?;
        match code {
            "0" | "2" => Some(TerminalSignal::Title(clean_text(payload))),
            "7" => {
                let uri = url::Url::parse(payload).ok()?;
                if uri.scheme() != "file" {
                    return None;
                }
                // Unix shells normally include the local hostname in OSC 7.
                // On Windows preserve the host, which can represent a UNC path.
                #[cfg(unix)]
                let path = std::path::PathBuf::from(
                    percent_encoding::percent_decode_str(uri.path())
                        .decode_utf8()
                        .ok()?
                        .as_ref(),
                );
                #[cfg(not(unix))]
                let path = uri.to_file_path().ok()?;
                Some(TerminalSignal::WorkingDirectory(
                    path.to_string_lossy().into_owned(),
                ))
            }
            "777" => {
                let rest = payload.strip_prefix("notify;")?;
                let (title, body) = rest.split_once(';').unwrap_or((rest, ""));
                notification(clean_text(title), clean_text(body))
            }
            // ConEmu uses OSC 9;4 and 9;9 for progress and CWD, respectively.
            "9" if !payload.starts_with("4;") && !payload.starts_with("9;") => {
                notification("Notification".into(), clean_text(payload))
            }
            "99" => self.parse_kitty(payload),
            _ => None,
        }
    }

    fn parse_kitty(&mut self, payload: &str) -> Option<TerminalSignal> {
        let (metadata, text) = payload.split_once(';')?;
        let fields: BTreeMap<_, _> = metadata
            .split(':')
            .filter_map(|field| field.split_once('='))
            .collect();
        let kind = fields.get("p").copied().unwrap_or("title");
        if !matches!(kind, "title" | "body") {
            return None;
        }
        let id = fields.get("i").copied().unwrap_or("").to_string();
        if !self.kitty.contains_key(&id) && self.kitty.len() >= MAX_PENDING_NOTIFICATIONS {
            self.kitty.pop_first();
        }
        let text = if fields.get("e") == Some(&"1") {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(text)
                .ok()?;
            clean_text(&String::from_utf8_lossy(&decoded))
        } else {
            clean_text(text)
        };
        let entry = self.kitty.entry(id.clone()).or_default();
        let part = if kind == "body" {
            &mut entry.1
        } else {
            &mut entry.0
        };
        if part.len().saturating_add(text.len()) > MAX_OSC_BYTES {
            self.kitty.remove(&id);
            return None;
        }
        part.push_str(&text);
        if fields.get("d") == Some(&"0") {
            return None;
        }
        let (title, body) = self.kitty.remove(&id)?;
        notification(title, body)
    }
}

fn clean_text(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .collect()
}

fn notification(title: String, body: String) -> Option<TerminalSignal> {
    if title.is_empty() && body.is_empty() {
        return None;
    }
    let title = if title.is_empty() {
        body.clone()
    } else {
        title
    };
    Some(TerminalSignal::Notification { title, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_survive_every_read_boundary_including_utf8_and_st() {
        let bytes =
            "before\x1b]777;notify;Ready 🦀;Review;please\x1b\\after\x1b]9;Done\x07".as_bytes();
        for split in 0..=bytes.len() {
            let mut parser = TerminalProtocolParser::default();
            let mut actual = parser.feed(&bytes[..split]);
            actual.extend(parser.feed(&bytes[split..]));
            assert_eq!(
                actual,
                vec![
                    TerminalSignal::Notification {
                        title: "Ready 🦀".into(),
                        body: "Review;please".into()
                    },
                    TerminalSignal::Notification {
                        title: "Notification".into(),
                        body: "Done".into()
                    },
                ],
                "split {split}"
            );
        }
    }

    #[test]
    fn kitty_chunks_and_interleaved_ids_follow_protocol() {
        let mut parser = TerminalProtocolParser::default();
        let output = parser
            .feed(b"\x1b]99;i=a:d=0;Hello\x07\x1b]99;i=b;Other\x07\x1b]99;i=a:p=body;world\x07");
        assert_eq!(
            output,
            vec![
                TerminalSignal::Notification {
                    title: "Other".into(),
                    body: "".into()
                },
                TerminalSignal::Notification {
                    title: "Hello".into(),
                    body: "world".into()
                },
            ]
        );
        assert_eq!(
            parser.feed(b"\x1b]99;e=1;SGVsbG8=\x07"),
            vec![TerminalSignal::Notification {
                title: "Hello".into(),
                body: "".into()
            }]
        );
    }

    #[test]
    fn unrelated_and_cancelled_osc_are_ignored() {
        let mut parser = TerminalProtocolParser::default();
        assert!(
            parser
                .feed(b"\x1b]9;4;1;50\x07\x1b]9;9;path\x07\x1b]99;p=?;\x07\x1b]9;cancelled\x18")
                .is_empty()
        );
        assert_eq!(
            parser.feed(b"\x1b]2;title\x07"),
            vec![TerminalSignal::Title("title".into())]
        );
    }

    #[test]
    fn oversized_and_incomplete_osc_do_not_grow_without_bound() {
        let mut parser = TerminalProtocolParser::default();
        parser.feed(b"\x1b]9;");
        assert!(parser.feed(&vec![b'x'; MAX_OSC_BYTES * 3]).is_empty());
        assert!(parser.payload.len() <= MAX_OSC_BYTES);
        assert_eq!(parser.feed(b"\x1b]9;recovered\x07").len(), 1);
        for i in 0..100 {
            parser.feed(format!("\x1b]99;i={i}:d=0;waiting\x07").as_bytes());
        }
        assert!(parser.kitty.len() <= MAX_PENDING_NOTIFICATIONS);
    }

    #[test]
    fn directory_uri_is_decoded_and_non_file_urls_are_rejected() {
        let mut parser = TerminalProtocolParser::default();
        let expected = url::Url::parse("file:///tmp/my%20project")
            .unwrap()
            .to_file_path()
            .unwrap();
        assert_eq!(
            parser.feed(b"\x1b]7;file:///tmp/my%20project\x07"),
            vec![TerminalSignal::WorkingDirectory(
                expected.to_string_lossy().into_owned()
            )]
        );
        assert!(parser.feed(b"\x1b]7;https://example.com/\x07").is_empty());
        #[cfg(unix)]
        assert_eq!(
            parser.feed(b"\x1b]7;file://my-machine.local/tmp/project\x07"),
            vec![TerminalSignal::WorkingDirectory("/tmp/project".into())]
        );
    }
}
