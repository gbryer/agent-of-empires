//! xtask - Development tasks for agent-of-empires

use clap::{CommandFactory, Parser, Subcommand};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Development tasks for agent-of-empires")]
struct Xtask {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate CLI documentation from clap definitions
    GenDocs,
    /// Check that contrib skill files reference valid CLI commands
    CheckSkill,
    /// Validate the embedded sbx kit spec.yaml is well-formed and idempotency-guarded
    CheckKit {
        /// Optional path to a spec.yaml fixture; defaults to the project's
        /// embedded src/containers/sbx_kit/spec.yaml.
        // `spec_path` is a clap arg name; the leading `--` in the CLI flag is
        // unavoidable and is not prose.
        #[arg(long)]
        spec_path: Option<std::path::PathBuf>,
    },
}

fn main() {
    let args = Xtask::parse();
    match args.command {
        Commands::GenDocs => generate_cli_docs(),
        Commands::CheckSkill => check_skill(),
        Commands::CheckKit { spec_path } => check_kit(spec_path.as_deref()),
    }
}

fn generate_cli_docs() {
    let markdown = clap_markdown::help_markdown::<agent_of_empires::cli::Cli>();

    let docs_dir = Path::new("docs/cli");
    fs::create_dir_all(docs_dir).expect("Failed to create docs/cli directory");

    let output_path = docs_dir.join("reference.md");
    fs::write(&output_path, markdown).expect("Failed to write CLI reference");

    println!("Generated CLI documentation at {}", output_path.display());
}

fn collect_subcommand_paths(cmd: &clap::Command, prefix: &str, out: &mut BTreeSet<String>) {
    for sub in cmd.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        let path = if prefix.is_empty() {
            sub.get_name().to_string()
        } else {
            format!("{} {}", prefix, sub.get_name())
        };
        out.insert(path.clone());
        collect_subcommand_paths(sub, &path, out);
    }
}

fn check_skill() {
    let skill_path = Path::new("contrib/openclaw-skill/SKILL.md");
    if !skill_path.exists() {
        eprintln!("Skill file not found: {}", skill_path.display());
        std::process::exit(1);
    }

    let content = fs::read_to_string(skill_path).expect("Failed to read SKILL.md");

    let mut has_error = false;

    // The skill's published version is managed by clawhub via _meta.json and
    // the release workflow's `--version` flag. A static `version:` in the
    // frontmatter goes stale on every release, so disallow it.
    if let Some((frontmatter, _)) = content
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---"))
    {
        for line in frontmatter.lines() {
            if line.starts_with("version:") {
                eprintln!(
                    "ERROR: SKILL.md frontmatter must not contain a top-level `version:` field; \
                     clawhub's _meta.json is the source of truth"
                );
                has_error = true;
                break;
            }
        }
    }

    // Build the clap command tree
    let cli_cmd = agent_of_empires::cli::Cli::command();
    let mut cli_commands: BTreeSet<String> = BTreeSet::new();
    collect_subcommand_paths(&cli_cmd, "", &mut cli_commands);

    // Extract `aoe <words>` patterns and match longest valid subcommand path
    let re = regex::Regex::new(r"aoe\s+([a-z][a-z0-9 -]*)").unwrap();
    let mut skill_commands: BTreeSet<String> = BTreeSet::new();
    for cap in re.captures_iter(&content) {
        let raw = cap[1].trim();
        let words: Vec<&str> = raw
            .split_whitespace()
            .take_while(|w| {
                !w.starts_with('-')
                    && !w.starts_with('<')
                    && !w.starts_with('"')
                    && !w.starts_with('$')
                    && !w.starts_with('/')
                    && !w.starts_with('.')
                    && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
            })
            .collect();

        // Find the longest prefix that is a known CLI command
        let mut best = String::new();
        let mut path = String::new();
        for word in &words {
            if path.is_empty() {
                path = word.to_string();
            } else {
                path = format!("{} {}", path, word);
            }
            if cli_commands.contains(&path) {
                best = path.clone();
            }
        }
        // If no exact match, use the first word if it's a known top-level command
        if best.is_empty() && !words.is_empty() && cli_commands.contains(words[0]) {
            best = words[0].to_string();
        }
        if !best.is_empty() {
            skill_commands.insert(best);
        }
    }

    // Check for skill references to commands that don't exist
    for skill_cmd in &skill_commands {
        if !cli_commands.contains(skill_cmd) {
            let is_prefix = cli_commands
                .iter()
                .any(|c| c.starts_with(&format!("{} ", skill_cmd)));
            if !is_prefix {
                eprintln!(
                    "ERROR: Skill references command 'aoe {}' which does not exist in CLI",
                    skill_cmd
                );
                has_error = true;
            }
        }
    }

    // Advisory: CLI commands not mentioned in skill
    let mut missing_from_skill = Vec::new();
    for cli_cmd in &cli_commands {
        let mentioned = skill_commands.iter().any(|s| {
            s == cli_cmd
                || cli_cmd.starts_with(&format!("{} ", s))
                || s.starts_with(&format!("{} ", cli_cmd))
        });
        if !mentioned {
            missing_from_skill.push(cli_cmd.clone());
        }
    }

    if !missing_from_skill.is_empty() {
        println!("Advisory: CLI commands not referenced in skill file:");
        for cmd in &missing_from_skill {
            println!("  aoe {}", cmd);
        }
    }

    if has_error {
        std::process::exit(1);
    }

    println!("Skill check passed.");
}

/// Naked install verbs whose presence in a `commands.{install,startup}` entry
/// must be paired with an idempotency guard (see `GUARD_PREFIXES`).
const NAKED_INSTALL_VERBS: &[&str] = &[
    "apt-get install",
    "apt install",
    "apt-get update",
    "apt update",
    "dpkg -i",
    "dpkg --install",
    "pip install",
    "pip3 install",
    "pip2 install",
    "npm install -g",
    "npm i -g",
    "yarn global add",
    "yarn add",
    "curl ",
    "wget ",
    "gem install",
];

/// Prefixes that, when they begin a trimmed command, are recognized as
/// idempotency guards. The lint also recognizes `||` and `&& ` anywhere in
/// the command, on the theory that any such operator means the install verb
/// is conditional on a prior check.
const GUARD_PREFIXES: &[&str] = &[
    "[ -f",
    "[ -x",
    "[ -e",
    "[ -d",
    "test -f",
    "test -x",
    "test -e",
    "test -d",
    "command -v",
    "which ",
];

fn is_idempotency_guarded(command: &str) -> bool {
    let trimmed = command.trim_start();
    if GUARD_PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
        return true;
    }
    command.contains(" || ") || command.contains("&& ")
}

/// Extract the shell command from a `command:` YAML value. Accepts either
/// `command: "string"` or `command: ["sh", "-c", "..."]` (and their longer
/// variants). For array forms whose first two elements are `["sh", "-c"]`,
/// returns the joined remainder so the lint operates on what the shell sees.
fn extract_command_string(value: &serde_yaml::Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    let seq = value.as_sequence()?;
    let parts: Vec<&str> = seq.iter().filter_map(|v| v.as_str()).collect();
    if parts.len() < 2 {
        return Some(parts.join(" "));
    }
    if parts[0] == "sh" && parts[1] == "-c" {
        Some(parts[2..].join(" "))
    } else {
        Some(parts.join(" "))
    }
}

fn lint_command_for_naked_install_verbs(command: &str, has_error: &mut bool) {
    if is_idempotency_guarded(command) {
        return;
    }
    for verb in NAKED_INSTALL_VERBS {
        if command.contains(verb) {
            eprintln!(
                "check-kit: naked install verb `{}` in commands.startup without idempotency guard:\n  {}",
                verb, command
            );
            *has_error = true;
        }
    }
}

fn require_str_field(
    spec: &serde_yaml::Value,
    field: &str,
    has_error: &mut bool,
    expected: Option<&str>,
) {
    let actual = spec.get(field).and_then(|v| v.as_str());
    match (actual, expected) {
        (Some(s), Some(exp)) if s != exp => {
            eprintln!(
                "check-kit: field `{}` must be the string {:?}, got {:?}",
                field, exp, s
            );
            *has_error = true;
        }
        (Some(_), _) => {}
        (None, Some(exp)) => {
            eprintln!(
                "check-kit: missing required field `{}` (must be the string {:?})",
                field, exp
            );
            *has_error = true;
        }
        (None, None) => {
            eprintln!("check-kit: missing required field `{}`", field);
            *has_error = true;
        }
    }
}

fn require_sequence_field(
    spec: &serde_yaml::Value,
    parent: &str,
    field: &str,
    has_error: &mut bool,
) -> Option<Vec<serde_yaml::Value>> {
    let seq = spec
        .get(parent)
        .and_then(|p| p.get(field))
        .and_then(|v| v.as_sequence())
        .cloned();
    if seq.is_none() {
        eprintln!(
            "check-kit: missing required field `{}.{}` (must be a YAML sequence)",
            parent, field
        );
        *has_error = true;
    }
    seq
}

fn lint_commands(entries: &[serde_yaml::Value], has_error: &mut bool) {
    for entry in entries {
        if let Some(cmd_value) = entry.get("command") {
            if let Some(cmd_str) = extract_command_string(cmd_value) {
                lint_command_for_naked_install_verbs(&cmd_str, has_error);
            }
        }
    }
}

fn check_kit(spec_path: Option<&std::path::Path>) {
    // The embedded spec.yaml is always available at compile time; an explicit
    // --spec-path overrides it for fixture-driven tests.
    let content = match spec_path {
        Some(p) => match fs::read_to_string(p) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("check-kit: failed to read {}: {}", p.display(), e);
                std::process::exit(1);
            }
        },
        None => include_str!("../../src/containers/sbx_kit/spec.yaml").to_string(),
    };

    let spec: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("check-kit: failed to parse spec.yaml: {}", e);
            std::process::exit(1);
        }
    };

    let mut has_error = false;

    require_str_field(&spec, "schemaVersion", &mut has_error, Some("1"));
    require_str_field(&spec, "kind", &mut has_error, None);
    require_str_field(&spec, "name", &mut has_error, None);

    if let Some(install) = require_sequence_field(&spec, "commands", "install", &mut has_error) {
        lint_commands(&install, &mut has_error);
    }
    if let Some(startup) = require_sequence_field(&spec, "commands", "startup", &mut has_error) {
        lint_commands(&startup, &mut has_error);
    }

    if has_error {
        std::process::exit(1);
    }
    println!("check-kit: OK");
}
