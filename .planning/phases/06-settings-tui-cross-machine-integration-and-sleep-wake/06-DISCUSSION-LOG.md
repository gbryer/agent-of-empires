# Phase 6: Settings TUI, Cross-Machine Integration, and Sleep/Wake - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 06-settings-tui-cross-machine-integration-and-sleep-wake
**Areas discussed:** Field visibility gating, Sleep/wake handler, Cross-machine verification, E2E test strategy

---

## Field Visibility Gating

| Option | Description | Selected |
|--------|-------------|----------|
| Test-only guard | Cross-product unit test documents field/runtime mapping. No TUI rendering changes. | |
| Skip input only | One is_field_editable(key, runtime) -> bool function. Input handler ignores keystrokes. | |
| Filter field list | build_sandbox_fields() excludes fields irrelevant to the resolved runtime. | ✓ |

**User's choice:** "Do what you think is best" — Claude selected filter field list.
**Notes:** User emphasized minimal changes for a public PR. No new rendering states, no greyed-out fields. The filter-at-build-time approach was chosen as the smallest diff that achieves capability gating.

---

## Sleep/Wake Handler

| Option | Description | Selected |
|--------|-------------|----------|
| Detection + resync call | Register for power notifications, run sbx exec date on wake. | ✓ |
| Scaffold only | Function signatures and hook points, no actual exec call. | |
| Mark-only approach | Mark sessions as clock-drifted, show warning badge. | |

**User's choice:** Full working handler (detection + resync call).
**Notes:** User confirmed clock drift is a real issue they've encountered. Key clarification: only resync aoe-managed sbx sandboxes (from sessions.json), not all sbx sandboxes on the system.

---

## Cross-Machine Verification

**User's choice:** Claude's discretion (no question asked — user let Claude decide).
**Notes:** Determined this is a verification + fix-if-needed concern. PTY relay and cockpit are runtime-agnostic by architecture. No preemptive code changes; researcher traces paths during planning.

---

## E2E Test Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| TUI selector only | Navigate settings, confirm Sbx in dropdown, test persistence. | |
| Skip e2e for now | Cross-product unit test covers correctness. Defer e2e. | |
| Full gated suite | #[ignore]-gated e2e covering selector + session create. | ✓ |

**User's choice:** "We should probably do whatever the lib does already" — follow existing patterns.
**Notes:** Existing Docker tests use #[ignore] gating in tests/e2e/ with TuiTestHarness. Sbx tests follow the same pattern. No new infrastructure.

---

## Claude's Discretion

- Exact IOKit/D-Bus registration boilerplate
- Whether wake handler uses std::thread or tokio::spawn_blocking
- Which sandbox fields are currently capability-gated vs not (audit during planning)
- Web integration test file location
- Cross-machine verification approach (determined to be verification-only, no preemptive changes)

## Deferred Ideas

- Visual greyed-out rendering for capability-gated fields (revisit if user confusion emerges)
- Richer diagnostic surface for hidden fields (aoe doctor enhancement)
- Wake handler for Docker/Podman sessions (not needed; they share host clock)
- Gitconfig identity / agent-config dirs (still deferred from Phase 4)
