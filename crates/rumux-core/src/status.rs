use serde::{Deserialize, Serialize};

/// Explicit activity reported by shell integration or an agent. Output alone
/// does not distinguish a running job from a prompt waiting for input.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceStatus {
    #[default]
    Idle,
    Working,
    Blocked,
    Exited,
}

impl SurfaceStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Working => "Working",
            Self::Blocked => "Needs input",
            Self::Exited => "Exited",
        }
    }
}
