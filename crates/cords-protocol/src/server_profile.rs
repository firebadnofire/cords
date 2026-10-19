//! Server-owned presentation. It never changes server identity or admission.
use crate::messaging::Statement;
use crate::profiles::{MAX_IMAGE, ProfileAvatar};
use serde::{Deserialize, Serialize};

pub const FEATURE: &str = "server-profile-v1";

#[must_use]
pub fn valid_background(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerProfile {
    pub version: u16,
    pub server_id: String,
    pub revision: u64,
    /// Empty at revision zero: use the operator-configured discovery name.
    pub name: String,
    pub description: String,
    pub background: String,
    pub icon: Option<ProfileAvatar>,
}
impl Statement for ServerProfile {
    const DOMAIN: &'static str = "CORDS-SERVER-PROFILE-V1";
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerIconChange {
    pub request_id: String,
    pub expected_revision: u64,
    pub icon: Option<ProfileAvatar>,
    pub image: Option<String>,
}

#[must_use]
pub fn valid_icon(icon: &ProfileAvatar) -> bool {
    crate::messaging::decode(&icon.sha256).is_ok_and(|value| value.len() == 32)
        && icon.bytes > 0
        && icon.bytes <= MAX_IMAGE
        && icon.width > 0
        && icon.width <= 1024
        && icon.height == icon.width
        && matches!(icon.shape.as_str(), "circle" | "square")
}

impl ServerIconChange {
    #[must_use]
    pub fn valid(&self) -> bool {
        !self.request_id.is_empty()
            && self.request_id.len() <= 128
            && match (&self.icon, &self.image) {
                (Some(icon), Some(image)) => valid_icon(icon) && image.len() <= 6_666_668,
                (None, None) => true,
                _ => false,
            }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerProfileChange {
    pub request_id: String,
    pub expected_revision: u64,
    pub name: String,
    pub description: String,
    pub background: String,
}

impl ServerProfileChange {
    #[must_use]
    pub fn valid(&self) -> bool {
        !self.request_id.is_empty()
            && valid_background(&self.background)
            && self.request_id.len() <= 128
            && !self.name.trim().is_empty()
            && self.name.chars().count() <= 100
            && !self.name.chars().any(char::is_control)
            && self.description.chars().count() <= 2000
            && !self
                .description
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_is_an_opaque_rgb_color() {
        for value in ["#000000", "#ffffff", "#aB12Cd"] {
            assert!(valid_background(value));
        }
        for value in [
            "",
            "red",
            "#123",
            "#12345g",
            "#12345678",
            "#123456\n",
            "#１２３４５６",
        ] {
            assert!(!valid_background(value));
        }
    }
}
