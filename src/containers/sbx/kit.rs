//! Materializer for the single aoe-authored sbx kit (CONTEXT.md D-08..D-13).
//!
//! Embeds seven static asset files from `src/containers/sbx_kit/` at compile
//! time via stdlib `include_str!`, then materializes a per-agent kit dir on
//! first sbx session via atomic-rename. The cache path encodes
//! `aoe-<CARGO_PKG_VERSION>-<sha256-prefix>` so a kit-bytes flip lands in a
//! new dir without mutating the prior one; concurrent racers either hit the
//! fast-path or harmlessly discard their tmp dir post-rename.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

const SPEC_YAML: &str = include_str!("../sbx_kit/spec.yaml");
const HOOK_SHIM: &str = include_str!("../sbx_kit/hook-shim.sh");
const GITIGNORE_AOE_HOOKS: &str = include_str!("../sbx_kit/gitignore-aoe-hooks");

const SETTINGS_CLAUDE: &str = include_str!("../sbx_kit/settings-claude.json");
const SETTINGS_GEMINI: &str = include_str!("../sbx_kit/settings-gemini.json");
const SETTINGS_CURSOR: &str = include_str!("../sbx_kit/settings-cursor.json");
const SETTINGS_QWEN: &str = include_str!("../sbx_kit/settings-qwen.json");

/// Agents whose `install_hint` cannot be applied headlessly inside an sbx
/// microVM (Cursor and Copilot are doc-only pointers; settl is a Homebrew
/// Cask). `ensure` fails loud at host time per CONTEXT.md D-10.
const UNSUPPORTED_AGENTS: &[&str] = &["cursor", "copilot", "settl"];

fn embedded_kit_bytes() -> Vec<u8> {
    let mut buf =
        Vec::with_capacity(SPEC_YAML.len() + HOOK_SHIM.len() + GITIGNORE_AOE_HOOKS.len() + 4096);
    buf.extend_from_slice(SPEC_YAML.as_bytes());
    buf.extend_from_slice(HOOK_SHIM.as_bytes());
    buf.extend_from_slice(GITIGNORE_AOE_HOOKS.as_bytes());
    buf.extend_from_slice(SETTINGS_CLAUDE.as_bytes());
    buf.extend_from_slice(SETTINGS_GEMINI.as_bytes());
    buf.extend_from_slice(SETTINGS_CURSOR.as_bytes());
    buf.extend_from_slice(SETTINGS_QWEN.as_bytes());
    buf
}

/// Cache-dir path for `(app_dir, agent)`, content-addressed by the embedded
/// kit-bytes SHA256 truncated to 16 lowercase-hex chars (CONTEXT.md D-08).
pub(crate) fn cache_dir(app_dir: &Path, agent: &str) -> PathBuf {
    cache_dir_inner(app_dir, agent, &embedded_kit_bytes())
}

fn cache_dir_inner(app_dir: &Path, agent: &str, bytes: &[u8]) -> PathBuf {
    let digest = Sha256::digest(bytes);
    let hash_hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    let segment = format!("aoe-{}-{}", env!("CARGO_PKG_VERSION"), &hash_hex[..16]);
    app_dir.join("sbx-kit").join(segment).join(agent)
}

/// Materialize the per-agent kit dir on first sbx session for `agent`,
/// returning the absolute path; a cache hit is a no-op fast path.
pub fn ensure(agent: &str) -> Result<PathBuf> {
    let app_dir = crate::session::get_app_dir()?;
    ensure_with_app_dir(&app_dir, agent)
}

fn ensure_with_app_dir(app_dir: &Path, agent: &str) -> Result<PathBuf> {
    let target = cache_dir(app_dir, agent);
    if target.exists() {
        return Ok(target);
    }

    if UNSUPPORTED_AGENTS.contains(&agent) {
        anyhow::bail!(
            "agent '{}' has no headless install path for sbx kit v1; install on the host and re-run",
            agent
        );
    }

    let install_body = crate::agents::install_hint(agent).ok_or_else(|| {
        anyhow::anyhow!(
            "agent '{}' has no install_hint; not supported in sbx kit v1",
            agent
        )
    })?;

    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cache_dir has no parent: {}", target.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create_dir_all {}", parent.display()))?;

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = parent.join(format!("{}.tmp.{}.{}", agent, std::process::id(), nanos));
    std::fs::create_dir_all(&tmp).with_context(|| format!("create_dir_all {}", tmp.display()))?;

    std::fs::write(tmp.join("spec.yaml"), SPEC_YAML.as_bytes())
        .with_context(|| format!("write spec.yaml in {}", tmp.display()))?;

    let install_sh = format!("#!/bin/sh\nset -eu\n{}\n", install_body);
    let install_path = tmp.join("install.sh");
    std::fs::write(&install_path, install_sh.as_bytes())
        .with_context(|| format!("write install.sh in {}", tmp.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&install_path, std::fs::Permissions::from_mode(0o755))
            .with_context(|| format!("chmod 0o755 {}", install_path.display()))?;
    }

    std::fs::write(tmp.join("hook-shim.sh"), HOOK_SHIM.as_bytes())
        .with_context(|| format!("write hook-shim.sh in {}", tmp.display()))?;
    std::fs::write(
        tmp.join("gitignore-aoe-hooks"),
        GITIGNORE_AOE_HOOKS.as_bytes(),
    )
    .with_context(|| format!("write gitignore-aoe-hooks in {}", tmp.display()))?;

    match std::fs::rename(&tmp, &target) {
        Ok(()) => {}
        Err(_) if target.exists() => {
            let _ = std::fs::remove_dir_all(&tmp);
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(anyhow::Error::from(e).context(format!(
                "rename {} -> {}",
                tmp.display(),
                target.display()
            )));
        }
    }

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_app_dir() -> tempfile::TempDir {
        tempfile::TempDir::new().unwrap()
    }

    #[test]
    fn cache_dir_path_shape() {
        let dir = temp_app_dir();
        let p = cache_dir(dir.path(), "claude");
        let s = p.to_string_lossy().to_string();
        let expected_prefix = format!("aoe-{}-", env!("CARGO_PKG_VERSION"));
        assert!(
            s.contains(&expected_prefix),
            "missing version prefix: {}",
            s
        );
        assert!(s.ends_with("/claude") || s.ends_with("/claude/"), "{}", s);

        // 16-hex segment after the version prefix.
        let segment = p
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let hash_part = segment.strip_prefix(&expected_prefix).unwrap_or("");
        assert_eq!(
            hash_part.len(),
            16,
            "hash segment length not 16: {:?}",
            hash_part
        );
        assert!(
            hash_part
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "hash segment not lowercase hex: {:?}",
            hash_part
        );
    }

    #[test]
    fn cache_dir_changes_on_content_byte_flip() {
        let dir = temp_app_dir();
        let a = cache_dir_inner(dir.path(), "claude", b"alpha");
        let b = cache_dir_inner(dir.path(), "claude", b"alphb");
        assert_ne!(a, b, "byte flip did not change cache dir: {:?}", a);
    }

    #[test]
    fn ensure_creates_files_on_miss() {
        let dir = temp_app_dir();
        let target = ensure_with_app_dir(dir.path(), "claude").unwrap();
        assert!(target.join("spec.yaml").exists(), "spec.yaml missing");
        assert!(target.join("install.sh").exists(), "install.sh missing");
        assert!(target.join("hook-shim.sh").exists(), "hook-shim.sh missing");
        assert!(
            target.join("gitignore-aoe-hooks").exists(),
            "gitignore-aoe-hooks missing"
        );
        let install_body = std::fs::read_to_string(target.join("install.sh")).unwrap();
        assert!(
            install_body.contains("npm install -g @anthropic-ai/claude-code"),
            "install.sh body missing claude install_hint: {}",
            install_body
        );
    }

    #[test]
    fn ensure_idempotent_on_hit() {
        let dir = temp_app_dir();
        let p1 = ensure_with_app_dir(dir.path(), "claude").unwrap();
        let mtime1 = std::fs::metadata(p1.join("spec.yaml"))
            .unwrap()
            .modified()
            .unwrap();
        let p2 = ensure_with_app_dir(dir.path(), "claude").unwrap();
        assert_eq!(p1, p2);
        let mtime2 = std::fs::metadata(p2.join("spec.yaml"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(mtime1, mtime2, "fast-path rewrote spec.yaml");
    }

    #[test]
    fn ensure_fails_loud_for_unsupported() {
        let dir = temp_app_dir();
        let err = ensure_with_app_dir(dir.path(), "cursor").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("cursor"),
            "error did not mention cursor: {}",
            msg
        );
        // No agent dir created when ensure fails fast.
        let agent_dir = cache_dir(dir.path(), "cursor");
        assert!(
            !agent_dir.exists(),
            "agent dir was created on unsupported agent: {}",
            agent_dir.display()
        );
    }

    #[test]
    fn install_sh_contains_install_hint_per_agent() {
        let dir = temp_app_dir();
        for agent in &[
            "claude", "codex", "opencode", "gemini", "droid", "pi", "qwen", "kiro", "hermes",
            "vibe",
        ] {
            let target = ensure_with_app_dir(dir.path(), agent).unwrap_or_else(|e| {
                panic!("ensure failed for v1 agent {}: {:#}", agent, e);
            });
            let body = std::fs::read_to_string(target.join("install.sh")).unwrap();
            let hint = crate::agents::install_hint(agent).unwrap();
            assert!(
                body.contains(hint),
                "install.sh for {} missing install_hint {:?}: {}",
                agent,
                hint,
                body
            );
        }
    }

    #[test]
    fn spec_yaml_hook_shim_uses_workspace_path() {
        let dir = temp_app_dir();
        let target = ensure_with_app_dir(dir.path(), "claude").unwrap();
        let spec_raw = std::fs::read_to_string(target.join("spec.yaml")).unwrap();
        let spec: serde_yaml::Value = serde_yaml::from_str(&spec_raw).unwrap();
        let init_files = spec
            .get("initFiles")
            .and_then(|v| v.as_sequence())
            .expect("initFiles missing");
        let any_matches = init_files.iter().any(|entry| {
            entry
                .get("content")
                .and_then(|c| c.as_str())
                .map(|s| s.contains("${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID/status"))
                .unwrap_or(false)
        });
        assert!(any_matches, "no initFiles content has WORKDIR-rooted shim");
    }

    #[test]
    fn spec_yaml_no_workdir_in_paths() {
        let dir = temp_app_dir();
        let target = ensure_with_app_dir(dir.path(), "claude").unwrap();
        let spec_raw = std::fs::read_to_string(target.join("spec.yaml")).unwrap();
        let spec: serde_yaml::Value = serde_yaml::from_str(&spec_raw).unwrap();
        for entry in spec
            .get("initFiles")
            .and_then(|v| v.as_sequence())
            .into_iter()
            .flatten()
        {
            let path = entry.get("path").and_then(|p| p.as_str()).unwrap_or("");
            assert!(
                !path.contains("${WORKDIR}"),
                "initFiles entry path contains WORKDIR placeholder: {}",
                path
            );
        }
    }

    #[test]
    fn spec_yaml_gitignore_drop_in_commands_install() {
        let dir = temp_app_dir();
        let target = ensure_with_app_dir(dir.path(), "claude").unwrap();
        let spec_raw = std::fs::read_to_string(target.join("spec.yaml")).unwrap();
        let spec: serde_yaml::Value = serde_yaml::from_str(&spec_raw).unwrap();
        let install_seq = spec["commands"]["install"]
            .as_sequence()
            .expect("commands.install must be a sequence");
        let mut combined = String::new();
        for entry in install_seq {
            if let Some(cmd) = entry.get("command") {
                if let Some(arr) = cmd.as_sequence() {
                    for piece in arr {
                        if let Some(s) = piece.as_str() {
                            combined.push_str(s);
                            combined.push('\n');
                        }
                    }
                } else if let Some(s) = cmd.as_str() {
                    combined.push_str(s);
                    combined.push('\n');
                }
            }
        }
        assert!(
            combined.contains(".aoe-hooks/"),
            "no .aoe-hooks/ in commands.install: {}",
            combined
        );
        assert!(
            combined.contains("grep -qsE"),
            "no idempotency guard in commands.install: {}",
            combined
        );
    }

    #[test]
    fn spec_yaml_declares_credentials_sources_ssh_agent() {
        let dir = temp_app_dir();
        let target = ensure_with_app_dir(dir.path(), "claude").unwrap();
        let spec_raw = std::fs::read_to_string(target.join("spec.yaml")).unwrap();
        let spec: serde_yaml::Value = serde_yaml::from_str(&spec_raw).unwrap();
        let sources = spec["credentials"]["sources"]
            .as_mapping()
            .expect("credentials.sources must be a mapping");
        // Walk leaves and assert at least one is the literal "ssh-agent".
        fn leaf_strings(v: &serde_yaml::Value, out: &mut Vec<String>) {
            match v {
                serde_yaml::Value::String(s) => out.push(s.clone()),
                serde_yaml::Value::Mapping(m) => {
                    for (_, vv) in m {
                        leaf_strings(vv, out);
                    }
                }
                serde_yaml::Value::Sequence(seq) => {
                    for vv in seq {
                        leaf_strings(vv, out);
                    }
                }
                _ => {}
            }
        }
        let mut leaves = Vec::new();
        leaf_strings(&serde_yaml::Value::Mapping(sources.clone()), &mut leaves);
        assert!(
            leaves.iter().any(|s| s == "ssh-agent"),
            "no ssh-agent leaf in credentials.sources: {:?}",
            leaves
        );
    }

    #[test]
    fn concurrent_ensure_all_succeed() {
        let dir = temp_app_dir();
        let p = dir.path().to_path_buf();
        let results: Vec<Result<PathBuf>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let p = p.clone();
                    s.spawn(move || ensure_with_app_dir(&p, "claude"))
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let paths: Vec<PathBuf> = results
            .into_iter()
            .map(|r| r.expect("concurrent ensure failed"))
            .collect();
        let first = &paths[0];
        for p2 in &paths {
            assert_eq!(p2, first, "racers returned different paths");
        }
        // No .tmp. dirs survive after all racers completed.
        let parent = first.parent().unwrap();
        let leftover_tmp: Vec<_> = parent
            .read_dir()
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(
            leftover_tmp.is_empty(),
            "tmp dirs survived: {:?}",
            leftover_tmp
                .iter()
                .map(|e| e.file_name())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn embedded_bytes_match_source() {
        let on_disk = std::fs::read_to_string("src/containers/sbx_kit/spec.yaml").unwrap();
        assert_eq!(SPEC_YAML, on_disk);
    }

    #[cfg(unix)]
    #[test]
    fn install_sh_is_executable_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_app_dir();
        let target = ensure_with_app_dir(dir.path(), "claude").unwrap();
        let perms = std::fs::metadata(target.join("install.sh"))
            .unwrap()
            .permissions();
        assert_eq!(perms.mode() & 0o777, 0o755, "install.sh mode wrong");
    }

    #[test]
    fn kit_path_threads_through_build_create_args() {
        use crate::containers::container_interface::ContainerConfig;
        use crate::containers::sbx::argv::build_create_args;
        let dir = temp_app_dir();
        let target = ensure_with_app_dir(dir.path(), "claude").unwrap();
        let cfg = ContainerConfig {
            working_dir: "/workspace".to_string(),
            volumes: vec![],
            anonymous_volumes: vec![],
            environment: vec![],
            cpu_limit: None,
            memory_limit: None,
            port_mappings: vec![],
        };
        let args = build_create_args("aoe-test", "alpine:latest", Some(&target), &cfg);
        let kit_pos = args
            .iter()
            .position(|a| a == "--kit")
            .expect("--kit missing");
        let tmpl_pos = args
            .iter()
            .position(|a| a == "--template")
            .expect("--template missing");
        assert_eq!(args[kit_pos + 1], target.display().to_string());
        assert_eq!(args[tmpl_pos + 1], "alpine:latest");
        assert!(kit_pos < tmpl_pos, "kit must precede template");
    }
}
