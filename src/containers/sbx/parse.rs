//! Serde structs for `sbx ls --json` output parsing (RT-02).
//!
//! Every field carries `#[serde(default)]` so malformed or partial responses
//! from future sbx versions deserialize without panic.

use serde::Deserialize;

/// Top-level wrapper around `sbx ls --json` output.
#[derive(Debug, Deserialize)]
pub struct SbxListOutput {
    #[serde(default)]
    pub sandboxes: Vec<SbxSandboxInfo>,
}

/// A single sandbox entry from `sbx ls --json`.
#[derive(Debug, Deserialize)]
pub struct SbxSandboxInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub socket_path: String,
    #[serde(default)]
    pub workspaces: Vec<String>,
}

/// Typed daemon health for richer diagnostics (D-05). Not on the trait;
/// sbx-specific callers use this for `aoe doctor` or settings hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SbxDaemonHealth {
    Running,
    NotLoggedIn,
    PolicyNotConfigured,
    NotInstalled,
    Unknown(String),
}

/// Find a sandbox by exact name in a parsed listing.
pub fn find_by_name<'a>(sandboxes: &'a [SbxSandboxInfo], name: &str) -> Option<&'a SbxSandboxInfo> {
    sandboxes.iter().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_output_shape() {
        let json = include_str!("../../../tests/fixtures/sbx_ls.json");
        let output: SbxListOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.sandboxes.len(), 1);
        let sb = &output.sandboxes[0];
        assert_eq!(sb.id, "edacdd7d-61a6-442b-a9a6-d8ccea8903ed");
        assert_eq!(sb.name, "claude-Lymow-HA");
        assert_eq!(sb.agent, "claude");
        assert_eq!(sb.status, "stopped");
        assert_eq!(sb.socket_path, "/tmp/sboxd-501-sandboxes/docker.sock");
        assert_eq!(
            sb.workspaces,
            vec!["/Users/gbryer/projects/other/lymow/Lymow-HA"]
        );
    }

    #[test]
    fn missing_fields_deserialize_with_defaults() {
        let json = "{}";
        let output: SbxSandboxInfo = serde_json::from_str(json).unwrap();
        assert_eq!(output.id, "");
        assert_eq!(output.name, "");
        assert_eq!(output.agent, "");
        assert_eq!(output.status, "");
        assert_eq!(output.socket_path, "");
        assert!(output.workspaces.is_empty());
    }

    #[test]
    fn find_by_name_returns_none_for_missing() {
        let json = include_str!("../../../tests/fixtures/sbx_ls.json");
        let output: SbxListOutput = serde_json::from_str(json).unwrap();
        assert!(find_by_name(&output.sandboxes, "nonexistent").is_none());
        assert!(find_by_name(&output.sandboxes, "claude-Lymow-HA").is_some());
    }
}
