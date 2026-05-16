//! `agent-of-empires remove` command implementation

use anyhow::{bail, Result};
use clap::Args;

use crate::session::deletion::{perform_deletion, DeletionRequest};
use crate::session::{GroupTree, Instance, Storage};

#[derive(Args)]
pub struct RemoveArgs {
    /// Session ID or title to remove
    identifier: String,

    /// Delete worktree directory (default: keep worktree)
    #[arg(long = "delete-worktree")]
    delete_worktree: bool,

    /// Delete git branch after worktree removal (default: per config)
    #[arg(long = "delete-branch")]
    delete_branch: bool,

    /// Force worktree removal even with untracked/modified files
    #[arg(long)]
    force: bool,

    /// Keep container instead of deleting it (default: delete per config)
    #[arg(long = "keep-container")]
    keep_container: bool,
}

/// Run on_destroy hooks attached to the controlling terminal (CLI context).
///
/// `perform_deletion` runs hooks detached (correct for TUI/web to avoid
/// stomping the rendered UI), but the CLI wants them attached so a hook
/// that legitimately needs user input can still be answered. We run them
/// here, then delegate to `perform_deletion` with `skip_hooks = true`.
fn run_on_destroy_hooks_attached(profile: &str, inst: &Instance) {
    let config = crate::session::repo_config::resolve_config_with_repo_or_warn(
        profile,
        std::path::Path::new(&inst.project_path),
    );
    let project_path = std::path::Path::new(&inst.project_path);
    let mut on_destroy = config.hooks.on_destroy.clone();

    // If the resolved config included repo hooks, verify they're still trusted.
    // Re-check trust to avoid running hooks that changed since approval.
    match crate::session::repo_config::check_hook_trust(project_path) {
        Ok(crate::session::repo_config::HookTrustStatus::Trusted(hooks))
            if !hooks.on_destroy.is_empty() =>
        {
            on_destroy = hooks.on_destroy.clone();
        }
        Ok(crate::session::repo_config::HookTrustStatus::NeedsTrust { .. }) => {
            // Repo hooks changed; fall back to global/profile only.
            on_destroy = crate::session::profile_config::resolve_config_or_warn(profile)
                .hooks
                .on_destroy;
        }
        _ => {}
    }

    if on_destroy.is_empty() {
        return;
    }

    let is_sandboxed = inst.sandbox_info.as_ref().is_some_and(|s| s.enabled);
    let errors = if is_sandboxed {
        if let Some(ref sandbox) = inst.sandbox_info {
            let workdir = inst.container_workdir_now();
            crate::session::repo_config::execute_hooks_in_container_best_effort(
                &on_destroy,
                &sandbox.container_name,
                &workdir,
                false,
            )
        } else {
            vec![]
        }
    } else {
        crate::session::repo_config::execute_hooks_best_effort(&on_destroy, project_path, false)
    };

    for err in &errors {
        eprintln!("Warning: on_destroy hook: {}", err);
    }
}

pub async fn run(profile: &str, args: RemoveArgs) -> Result<()> {
    let storage = Storage::new(profile)?;
    let (instances, groups) = storage.load_with_groups()?;

    let mut found = false;
    let mut removed_title = String::new();
    let mut new_instances = Vec::with_capacity(instances.len());

    for inst in instances {
        if inst.id == args.identifier
            || inst.id.starts_with(&args.identifier)
            || inst.title == args.identifier
        {
            found = true;
            removed_title = inst.title.clone();

            let config = crate::session::repo_config::resolve_config_with_repo_or_warn(
                profile,
                std::path::Path::new(&inst.project_path),
            );

            // Hooks first, attached to the terminal so prompts work.
            run_on_destroy_hooks_attached(profile, &inst);

            // Compute the shared-deletion flags from the CLI args + config.
            // `delete_worktree` only applies when the worktree is managed.
            let managed_worktree = inst
                .worktree_info
                .as_ref()
                .is_some_and(|wt| wt.managed_by_aoe);
            let managed_workspace = inst.workspace_info.as_ref().is_some_and(|ws| {
                ws.cleanup_on_delete && ws.repos.iter().any(|r| r.managed_by_aoe)
            });
            let delete_worktree = args.delete_worktree && (managed_worktree || managed_workspace);

            // Delete branch if explicitly requested, or if worktree is being
            // deleted and config says to also delete the branch. Mirrors the
            // pre-refactor cli/remove logic.
            let delete_branch = managed_worktree
                && (args.delete_branch
                    || (delete_worktree && config.worktree.delete_branch_on_cleanup));

            // --keep-container always preserves the container. Otherwise
            // honor `config.sandbox.auto_cleanup` (a user who disabled it
            // wants containers kept even on remove).
            let sandbox_enabled = inst.sandbox_info.as_ref().is_some_and(|s| s.enabled);
            let delete_sandbox =
                sandbox_enabled && !args.keep_container && config.sandbox.auto_cleanup;

            // Pre-message: tell the user what will happen for preserved bits
            // so the user-visible output stays close to the legacy CLI.
            if args.delete_worktree {
                // delete_worktree was requested but the worktree isn't aoe-
                // managed: tell the user we won't touch it.
                if !managed_worktree && inst.worktree_info.is_some() {
                    println!(
                        "Worktree preserved at: {} (not managed by aoe)",
                        inst.project_path
                    );
                }
            } else if managed_worktree {
                println!(
                    "Worktree preserved at: {} (use --delete-worktree to remove)",
                    inst.project_path
                );
            }
            if sandbox_enabled && args.keep_container {
                if let Some(s) = &inst.sandbox_info {
                    println!("Container preserved: {}", s.container_name);
                }
            } else if sandbox_enabled && !config.sandbox.auto_cleanup {
                if let Some(s) = &inst.sandbox_info {
                    println!(
                        "Container preserved: {} (auto_cleanup disabled in config)",
                        s.container_name
                    );
                }
            }

            let request = DeletionRequest {
                session_id: inst.id.clone(),
                instance: inst.clone(),
                delete_worktree,
                delete_branch,
                delete_sandbox,
                force_delete: args.force,
                skip_hooks: true,
            };
            let result = perform_deletion(&request);

            if delete_worktree {
                if result.success {
                    println!("  Worktree removed");
                } else if let Some(err) = result.error.as_ref() {
                    // Surface the underlying error message so the user has
                    // something to act on.
                    for line in err.split("; ") {
                        eprintln!("Warning: {}", line);
                    }
                    eprintln!(
                        "You may need to remove it manually with: git worktree remove {}",
                        inst.project_path
                    );
                }
            } else if !result.success {
                // Errors unrelated to worktree (container, branch, etc.).
                if let Some(err) = result.error.as_ref() {
                    for line in err.split("; ") {
                        eprintln!("Warning: {}", line);
                    }
                }
            }

            if delete_branch && result.success {
                if let Some(wt) = inst.worktree_info.as_ref() {
                    println!("  Branch '{}' deleted", wt.branch);
                }
            }

            if delete_sandbox && result.success {
                println!("  Container removed");
            }
        } else {
            new_instances.push(inst);
        }
    }

    if !found {
        bail!(
            "Session not found in profile '{}': {}",
            storage.profile(),
            args.identifier
        );
    }

    // Rebuild group tree and save
    let group_tree = GroupTree::new_with_groups(&new_instances, &groups);
    storage.save_with_groups(&new_instances, &group_tree)?;

    println!(
        "  Removed session: {} (from profile '{}')",
        removed_title,
        storage.profile()
    );

    Ok(())
}
