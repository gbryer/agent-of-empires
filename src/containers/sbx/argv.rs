//! Pure free-function argv builders for the Docker Sandboxes (`sbx`) CLI.
//!
//! Each builder returns a `Vec<String>` and never invokes a subprocess; the
//! returned vector is passed to `Command::args(&args)` by callers in plan
//! 02-02 (dispatch arms in `runtime.rs`) and Phase 5 (action verbs). The
//! analog is `RuntimeBase::build_create_args` at runtime_base.rs:223-317.

use std::path::Path;

use crate::containers::container_interface::{ContainerConfig, EnvEntry};

/// Build the argv tail for `sbx create shell ...`. The caller prefixes
/// the binary name (`sbx`) at invocation time.
///
/// `kit_path` is `Option<&Path>` because Phase 4 owns kit materialization
/// (CONTEXT.md D-08); plan 02-01 tests pass `None`.
///
/// sbx's volume model is positional (`PATH[:ro]`) rather than `-v
/// HOST:CONTAINER`. Phase 3 (RT-04) will enforce `host_path == container_path`
/// upstream; here we emit `vol.host_path` verbatim. Anonymous volumes and
/// `-p PORT` mappings are NOT emitted at create time (sbx capabilities
/// `supports_anonymous_volumes` and `supports_port_publish_at_create` are
/// both false); ports go through `build_ports_args` post-create.
pub fn build_create_args(
    name: &str,
    image: &str,
    kit_path: Option<&Path>,
    config: &ContainerConfig,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "create".to_string(),
        "shell".to_string(),
        "--name".to_string(),
        name.to_string(),
    ];

    if let Some(kp) = kit_path {
        args.push("--kit".to_string());
        args.push(kp.display().to_string());
    }

    args.push("--template".to_string());
    args.push(image.to_string());

    if let Some(cpu) = &config.cpu_limit {
        args.push("--cpus".to_string());
        args.push(cpu.clone());
    }
    if let Some(mem) = &config.memory_limit {
        args.push("-m".to_string());
        args.push(mem.clone());
    }

    for vol in &config.volumes {
        let suffix = if vol.read_only { ":ro" } else { "" };
        args.push(format!("{}{}", vol.host_path, suffix));
    }

    args
}

/// Build the argv tail for `sbx exec [flags] SANDBOX COMMAND [ARG...]`.
///
/// EnvEntry encoding mirrors `runtime_base.rs:276-287` exactly (Inherit emits
/// only the key in argv so secrets do not leak into `ps`; Literal emits
/// `KEY=VALUE`). PATTERNS.md Pattern B forbids extracting a shared helper
/// at this stage.
pub fn build_exec_args(
    name: &str,
    workdir: Option<&str>,
    env: &[EnvEntry],
    interactive: bool,
    tty: bool,
    cmd: &[&str],
) -> Vec<String> {
    let mut args: Vec<String> = vec!["exec".to_string()];

    if interactive {
        args.push("-i".to_string());
    }
    if tty {
        args.push("-t".to_string());
    }

    if let Some(wd) = workdir {
        args.push("-w".to_string());
        args.push(wd.to_string());
    }

    for entry in env {
        args.push("-e".to_string());
        match entry {
            EnvEntry::Inherit { key, .. } => args.push(key.clone()),
            EnvEntry::Literal { key, value } => args.push(format!("{}={}", key, value)),
        }
    }

    args.push(name.to_string());
    for c in cmd {
        args.push((*c).to_string());
    }

    args
}

/// Build the argv tail for a single `sbx ports SANDBOX --publish PORT_SPEC`
/// invocation. The per-port loop lives in Phase 5's `ports.rs`; this builder
/// is a pure pass-through and does not parse `port_spec`.
pub fn build_ports_args(name: &str, port_spec: &str) -> Vec<String> {
    vec![
        "ports".to_string(),
        name.to_string(),
        "--publish".to_string(),
        port_spec.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::container_interface::VolumeMount;

    fn empty_config() -> ContainerConfig {
        ContainerConfig {
            working_dir: "/workspace".to_string(),
            volumes: vec![],
            anonymous_volumes: vec![],
            environment: vec![],
            cpu_limit: None,
            memory_limit: None,
            port_mappings: vec![],
        }
    }

    // -------- build_create_args tests --------

    #[test]
    fn create_args_minimal() {
        let cfg = empty_config();
        let args = build_create_args("aoe-sandbox-test", "alpine:latest", None, &cfg);
        assert_eq!(
            args,
            vec![
                "create".to_string(),
                "shell".to_string(),
                "--name".to_string(),
                "aoe-sandbox-test".to_string(),
                "--template".to_string(),
                "alpine:latest".to_string(),
            ]
        );
        assert!(!args.iter().any(|a| a == "--kit"));
        assert!(!args.iter().any(|a| a == "--cpus"));
        assert!(!args.iter().any(|a| a == "-m"));
    }

    #[test]
    fn create_args_with_kit() {
        let cfg = empty_config();
        let kp = Path::new("/var/cache/aoe/sbx-kit/v1");
        let args = build_create_args("s", "alpine:latest", Some(kp), &cfg);
        let kit_pos = args.iter().position(|a| a == "--kit").unwrap();
        assert_eq!(args[kit_pos + 1], "/var/cache/aoe/sbx-kit/v1");
        let name_pos = args.iter().position(|a| a == "--name").unwrap();
        let tmpl_pos = args.iter().position(|a| a == "--template").unwrap();
        assert!(name_pos < kit_pos);
        assert!(kit_pos < tmpl_pos);
    }

    #[test]
    fn create_args_with_cpus_and_memory() {
        let mut cfg = empty_config();
        cfg.cpu_limit = Some("2".to_string());
        cfg.memory_limit = Some("4g".to_string());
        let args = build_create_args("s", "alpine:latest", None, &cfg);
        let tmpl_pos = args.iter().position(|a| a == "--template").unwrap();
        let cpus_pos = args.iter().position(|a| a == "--cpus").unwrap();
        let mem_pos = args.iter().position(|a| a == "-m").unwrap();
        assert_eq!(args[cpus_pos + 1], "2");
        assert_eq!(args[mem_pos + 1], "4g");
        assert!(tmpl_pos < cpus_pos);
        assert!(cpus_pos < mem_pos);
    }

    #[test]
    fn create_args_with_volume_no_ro() {
        let mut cfg = empty_config();
        cfg.volumes = vec![VolumeMount {
            host_path: "/work/proj".to_string(),
            container_path: "/work/proj".to_string(),
            read_only: false,
        }];
        let args = build_create_args("s", "alpine:latest", None, &cfg);
        assert_eq!(args.last().unwrap(), "/work/proj");
        assert!(!args.iter().any(|a| a.ends_with(":ro")));
    }

    #[test]
    fn create_args_with_volume_ro() {
        let mut cfg = empty_config();
        cfg.volumes = vec![VolumeMount {
            host_path: "/work/proj".to_string(),
            container_path: "/work/proj".to_string(),
            read_only: true,
        }];
        let args = build_create_args("s", "alpine:latest", None, &cfg);
        assert_eq!(args.last().unwrap(), "/work/proj:ro");
    }

    #[test]
    fn create_args_with_multiple_volumes() {
        let mut cfg = empty_config();
        cfg.volumes = vec![
            VolumeMount {
                host_path: "/a".to_string(),
                container_path: "/a".to_string(),
                read_only: false,
            },
            VolumeMount {
                host_path: "/b".to_string(),
                container_path: "/b".to_string(),
                read_only: true,
            },
        ];
        let args = build_create_args("s", "alpine:latest", None, &cfg);
        let a_pos = args.iter().position(|a| a == "/a").unwrap();
        let b_pos = args.iter().position(|a| a == "/b:ro").unwrap();
        assert!(a_pos < b_pos);
    }

    #[test]
    fn create_args_skips_anonymous_volumes() {
        let mut cfg = empty_config();
        cfg.anonymous_volumes = vec!["/tmp/cache".to_string()];
        let args = build_create_args("s", "alpine:latest", None, &cfg);
        assert!(!args.iter().any(|a| a == "/tmp/cache"));
    }

    #[test]
    fn create_args_skips_port_mappings() {
        let mut cfg = empty_config();
        cfg.port_mappings = vec!["3000:3000".to_string()];
        let args = build_create_args("s", "alpine:latest", None, &cfg);
        assert!(!args.iter().any(|a| a == "-p"));
        assert!(!args.iter().any(|a| a == "3000:3000"));
    }

    // -------- build_exec_args tests --------

    #[test]
    fn exec_args_minimal() {
        let args = build_exec_args("name", None, &[], false, false, &["sh", "-c", "echo hi"]);
        assert_eq!(
            args,
            vec![
                "exec".to_string(),
                "name".to_string(),
                "sh".to_string(),
                "-c".to_string(),
                "echo hi".to_string(),
            ]
        );
        assert!(!args.iter().any(|a| a == "-i"));
        assert!(!args.iter().any(|a| a == "-t"));
        assert!(!args.iter().any(|a| a == "-w"));
        assert!(!args.iter().any(|a| a == "-e"));
    }

    #[test]
    fn exec_args_interactive_only() {
        let args = build_exec_args("name", None, &[], true, false, &["echo"]);
        assert!(args.iter().any(|a| a == "-i"));
        assert!(!args.iter().any(|a| a == "-t"));
        let i_pos = args.iter().position(|a| a == "-i").unwrap();
        let name_pos = args.iter().position(|a| a == "name").unwrap();
        assert!(i_pos < name_pos);
    }

    #[test]
    fn exec_args_tty_only() {
        let args = build_exec_args("name", None, &[], false, true, &["echo"]);
        assert!(args.iter().any(|a| a == "-t"));
        assert!(!args.iter().any(|a| a == "-i"));
        let t_pos = args.iter().position(|a| a == "-t").unwrap();
        let name_pos = args.iter().position(|a| a == "name").unwrap();
        assert!(t_pos < name_pos);
    }

    #[test]
    fn exec_args_interactive_and_tty() {
        let args = build_exec_args("name", None, &[], true, true, &["echo"]);
        let i_pos = args.iter().position(|a| a == "-i").unwrap();
        let t_pos = args.iter().position(|a| a == "-t").unwrap();
        let name_pos = args.iter().position(|a| a == "name").unwrap();
        assert!(i_pos < t_pos);
        assert!(t_pos < name_pos);
    }

    #[test]
    fn exec_args_with_workdir() {
        let args = build_exec_args("name", Some("/workspace/proj"), &[], true, true, &["echo"]);
        let w_pos = args.iter().position(|a| a == "-w").unwrap();
        assert_eq!(args[w_pos + 1], "/workspace/proj");
        let t_pos = args.iter().position(|a| a == "-t").unwrap();
        let name_pos = args.iter().position(|a| a == "name").unwrap();
        assert!(t_pos < w_pos);
        assert!(w_pos < name_pos);
    }

    #[test]
    fn exec_args_with_env_inherit() {
        let env = vec![EnvEntry::Inherit {
            key: "GITHUB_TOKEN".to_string(),
            value: "ignored".to_string(),
        }];
        let args = build_exec_args("name", None, &env, false, false, &["echo"]);
        let e_pos = args.iter().position(|a| a == "-e").unwrap();
        assert_eq!(args[e_pos + 1], "GITHUB_TOKEN");
        assert!(!args.iter().any(|a| a == "ignored"));
        assert!(!args.iter().any(|a| a == "GITHUB_TOKEN=ignored"));
    }

    #[test]
    fn exec_args_with_env_literal() {
        let env = vec![EnvEntry::Literal {
            key: "FOO".to_string(),
            value: "bar".to_string(),
        }];
        let args = build_exec_args("name", None, &env, false, false, &["echo"]);
        let e_pos = args.iter().position(|a| a == "-e").unwrap();
        assert_eq!(args[e_pos + 1], "FOO=bar");
    }

    #[test]
    fn exec_args_with_mixed_env() {
        let env = vec![
            EnvEntry::Inherit {
                key: "K1".to_string(),
                value: "v1".to_string(),
            },
            EnvEntry::Literal {
                key: "K2".to_string(),
                value: "v2".to_string(),
            },
        ];
        let args = build_exec_args("name", None, &env, false, false, &["echo"]);
        let first_e = args.iter().position(|a| a == "-e").unwrap();
        assert_eq!(args[first_e + 1], "K1");
        let second_e = args
            .iter()
            .enumerate()
            .skip(first_e + 1)
            .find(|(_, a)| *a == "-e")
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(args[second_e + 1], "K2=v2");
    }

    #[test]
    fn exec_args_full_permutation() {
        let env = vec![
            EnvEntry::Inherit {
                key: "INH".to_string(),
                value: "x".to_string(),
            },
            EnvEntry::Literal {
                key: "LIT".to_string(),
                value: "y".to_string(),
            },
        ];
        let args = build_exec_args(
            "name",
            Some("/w"),
            &env,
            true,
            true,
            &["claude", "--continue"],
        );
        assert_eq!(
            args,
            vec![
                "exec".to_string(),
                "-i".to_string(),
                "-t".to_string(),
                "-w".to_string(),
                "/w".to_string(),
                "-e".to_string(),
                "INH".to_string(),
                "-e".to_string(),
                "LIT=y".to_string(),
                "name".to_string(),
                "claude".to_string(),
                "--continue".to_string(),
            ]
        );
    }

    // -------- build_ports_args tests --------

    #[test]
    fn ports_args_simple() {
        let args = build_ports_args("name", "3000:3000");
        assert_eq!(
            args,
            vec![
                "ports".to_string(),
                "name".to_string(),
                "--publish".to_string(),
                "3000:3000".to_string(),
            ]
        );
    }

    #[test]
    fn ports_args_with_host_ip() {
        let args = build_ports_args("name", "127.0.0.1:8080:80");
        assert_eq!(
            args,
            vec![
                "ports".to_string(),
                "name".to_string(),
                "--publish".to_string(),
                "127.0.0.1:8080:80".to_string(),
            ]
        );
    }

    #[test]
    fn ports_args_with_protocol() {
        let args = build_ports_args("name", "3000:3000/tcp");
        assert_eq!(
            args,
            vec![
                "ports".to_string(),
                "name".to_string(),
                "--publish".to_string(),
                "3000:3000/tcp".to_string(),
            ]
        );
    }
}
