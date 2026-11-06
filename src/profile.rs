//! Braid profile: the user-facing import format (JSON), not Clash YAML.

use serde::{Deserialize, Serialize};

pub const DEMO_PROFILE_JSON: &str = include_str!("../fixtures/demo.profile.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub server: String,
    pub port: u16,
    pub reality: Reality,
    pub hysteria2: Hysteria2,
    #[serde(default)]
    pub cidr_direct: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reality {
    pub uuid: String,
    pub public_key: String,
    pub short_id: String,
    pub server_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hysteria2 {
    pub password: String,
    pub obfs_password: String,
    pub server_name: String,
    #[serde(default)]
    pub pin_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    Json(String),
    Invalid(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileError::Json(msg) => write!(f, "invalid JSON: {msg}"),
            ProfileError::Invalid(msg) => write!(f, "invalid profile: {msg}"),
        }
    }
}

impl Profile {
    pub fn from_json(text: &str) -> Result<Self, ProfileError> {
        let profile: Self =
            serde_json::from_str(text).map_err(|e| ProfileError::Json(e.to_string()))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn demo() -> Self {
        Self::from_json(DEMO_PROFILE_JSON).expect("demo profile fixture must parse")
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.name.trim().is_empty() {
            return Err(ProfileError::Invalid("name is empty".into()));
        }
        if self.server.trim().is_empty() {
            return Err(ProfileError::Invalid("server is empty".into()));
        }
        if self.port == 0 {
            return Err(ProfileError::Invalid("port must be non-zero".into()));
        }
        if self.reality.uuid.trim().is_empty()
            || self.reality.public_key.trim().is_empty()
            || self.reality.short_id.trim().is_empty()
            || self.reality.server_name.trim().is_empty()
        {
            return Err(ProfileError::Invalid("reality fields incomplete".into()));
        }
        if self.hysteria2.password.trim().is_empty()
            || self.hysteria2.obfs_password.trim().is_empty()
            || self.hysteria2.server_name.trim().is_empty()
        {
            return Err(ProfileError::Invalid("hysteria2 fields incomplete".into()));
        }
        Ok(())
    }

    /// Prefer explicit cidr_direct; otherwise derive `{server}/32` when server looks like IPv4.
    pub fn direct_cidrs(&self) -> Vec<String> {
        if !self.cidr_direct.is_empty() {
            return self.cidr_direct.clone();
        }
        if is_ipv4(&self.server) {
            vec![format!("{}/32", self.server)]
        } else {
            Vec::new()
        }
    }
}

fn is_ipv4(host: &str) -> bool {
    let parts: Vec<_> = host.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts
        .iter()
        .all(|p| p.parse::<u8>().is_ok() && !p.is_empty() && p.len() <= 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_fixture_loads() {
        let p = Profile::demo();
        assert_eq!(p.name, "Demo node");
        assert_eq!(p.server, "203.0.113.10");
        assert_eq!(p.direct_cidrs(), vec!["203.0.113.10/32".to_owned()]);
    }

    #[test]
    fn rejects_empty_name() {
        let mut bad = DEMO_PROFILE_JSON.to_owned();
        bad = bad.replacen("Demo node", "", 1);
        assert!(Profile::from_json(&bad).is_err());
    }
}
