# Phase 05 Manual Test Plan

Manual walkthroughs for sbx sandbox integration. Actual execution is Phase 6
verification scope; this document captures what to test and expected behavior.

## Cockpit Session Creation

Steps:
1. Launch `aoe` TUI (cargo run or installed binary)
2. Navigate to the cockpit (main screen)
3. Press `a` to add a new session
4. Select a tool (e.g., claude)
5. Ensure `Settings > Sandbox > Container Runtime` is set to `sbx`
6. Create the session

Expected:
- TUI shows "Creating sandbox..." status during creation
- `sbx ls --json` shows the new sandbox with status "running"
- The session's tmux pane connects and the tool starts inside the sandbox
- Port mappings from settings are published (check `sbx ports <name>` output)
- `aoe` status bar shows the sandbox runtime indicator

Failure indicators:
- Error toast: "kit materialization failed" (sbx not logged in or policy missing)
- Error toast: "sandbox not ready after ~25s" (daemon health issue)
- Session starts but tool exits immediately (exec retry exhausted)

## Web Dashboard Session

Prerequisites: `cargo build --features serve && aoe serve --no-auth`

Steps:
1. Open web dashboard in browser (URL from `aoe serve` output)
2. Click "New Session" or use REST API: `POST /api/sessions`
3. Set body: `{"tool": "claude", "container_runtime": "sbx", "project_path": "/tmp/test"}`
4. Observe session creation via WebSocket status updates

Expected:
- API returns 201 with session ID
- WebSocket streams creation progress events
- Terminal connects to the sandbox once ready
- Dashboard session list shows the sbx runtime badge

## Cross-Machine Session

Prerequisites: two machines on the same network, sbx configured on remote

Steps:
1. On remote machine: `aoe serve --host 0.0.0.0`
2. Note the URL with auth token from output
3. On local machine: open the URL in browser
4. Create a new sbx session from the web dashboard
5. Verify the session runs inside the remote machine's sbx daemon

Expected:
- Session creates and connects over WebSocket relay
- Terminal latency is acceptable (< 200ms for keystrokes)
- Port publishing works and ports are accessible from local machine via tunnel
- Closing browser tab does not stop the sandbox (it persists on the remote)

## Sleep/Wake Clock Drift

Relevant to macOS laptops where sbx sandboxes survive system sleep.

Steps:
1. Create an sbx session via cockpit
2. Verify the session is running (exec a command)
3. Close laptop lid or `pmset sleepnow`
4. Wait 30+ seconds
5. Wake the machine
6. Attempt to use the session (type a command)

Expected:
- Sandbox resumes within 5-10 seconds of wake
- HTTPS connections inside the sandbox (e.g., API calls from claude) succeed
  after initial retry (TLS handshake may fail on first attempt due to clock skew)
- No "not running" errors after wake stabilization
- `sbx ls --json` shows status "running" after wake

Potential issues:
- Certificate validation failure if system clock drifts > 5 minutes during sleep
- First exec after wake triggers the retry logic (1s, 2s, 4s delays)

## Disk Usage Growth

Monitoring host disk consumption across sandbox lifecycle operations.

Steps:
1. Record baseline: `df -h` and `du -sh ~/Library/Containers/` (or equivalent sbx data dir)
2. Create 5 sbx sessions sequentially, each with a different tool
3. Record disk after creation: `df -h`
4. Remove all 5 sandboxes: `sbx rm -f <name>` for each
5. Record disk after removal: `df -h`
6. Compare baseline vs post-removal to check for leaked layers

Expected:
- Each sandbox creation adds 50-200MB (base image layer + kit)
- After removal, disk usage returns to within 10MB of baseline
- No orphaned mount points in `mount | grep sbx`
- Kit materialization caches are reused across same-agent sessions

Growth thresholds (warn if exceeded):
- Single sandbox > 500MB disk overhead
- Post-removal delta from baseline > 50MB (indicates leaked layers)
- More than 3 entries in sbx data dir after all sandboxes removed
