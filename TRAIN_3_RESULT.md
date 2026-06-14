# Train 3 Result — Worker / fleet / sub-agent convergence

Branch: `codex/v0.8.61-train-3`

## Status

Implemented and committed the highest-value Train 3 slices:

- `c497e51a8` — TUI input pump moved blocking terminal input to its own thread; AgentProgress drain is capped and coalesced. `Refs: #3216`
- `73ff50bd2` — sub-agents no longer park forever on `input_rx.recv()` after timeout; they return structured `needs_input` checkpoints. `Refs: #3096`
- `8e45d6569` — sidebar and tool projections use real `AgentWorkerStatus` values instead of hardcoded `running`. `Refs: #3226`
- `a59249d8b` — worker records expose recommended parent actions. `Refs: #3226`
- `43cfb6bcd` — `agent_open` / fleet worker records build and persist `WorkerRuntimeProfile` contracts. `Refs: #3217`
- `76af2a08d` — six-worker progress storm test covers bounded redraw, input delivery, and cancel liveness. `Refs: #3216`

No tags, release artifacts, version files, `Cargo.lock`, or pushes were created.

## Files

- `crates/tui/src/tui/ui.rs`
- `crates/tui/src/tui/ui/tests.rs`
- `crates/tui/src/tools/subagent/mod.rs`
- `crates/tui/src/tools/subagent/tests.rs`
- `crates/tui/src/tui/sidebar.rs`
- `crates/tui/src/tui/views/mod.rs`
- `crates/tui/src/fleet/worker_runtime.rs`
- `crates/tui/src/runtime_api.rs`
- `crates/tui/src/worker_profile.rs`

## Verification

Passed:

- `cargo fmt --all --check`
- `git diff --check`
- `cargo test -p codewhale-tui agent_progress_redraw --locked` — 2 passed
- `cargo test -p codewhale-tui six_worker_progress_storm_keeps_input_render_and_cancel_live --locked` — 1 passed
- `cargo test -p codewhale-tui sidebar_agent_rows --locked` — 2 passed
- `cargo test -p codewhale-tui sidebar_progress_only_rows_parse_status_instead_of_hardcoding_running --locked` — 1 passed
- `cargo test -p codewhale-tui agent_open_worker_profile_derives_from_parent_without_escalation --locked` — 1 passed
- `cargo test -p codewhale-tui headless_worker_record_tracks_lifecycle_without_tui_projection --locked` — 1 passed
- `cargo test -p codewhale-tui fleet_worker_spec_defaults_to_shared_subagent_spawn_depth --locked` — 1 passed
- `cargo test -p codewhale-tui agent_runs_runtime_api_exposes_persisted_worker_receipts --locked` — 1 passed

Flaky in this sandbox:

- `cargo test -p codewhale-tui api_timeout_preserves_checkpoint_and_returns_needs_input_without_parking --locked`
  - Passed once.
  - Also failed once waiting for the local fake chat server request and once with `127.0.0.1:0` bind `PermissionDenied`.
  - The failure mode is local test-server/sandbox timing, not an observed regression in the committed code.

## Risks / Remaining

- `agent_open` now records a `WorkerRuntimeProfile`, but default sub-agent execution is still in-process. Full durable fleet enqueue for `agent_open` remains the larger #3096/#3154 follow-up.
- Runtime profile enforcement is partial: the profile is built, intersected, persisted, and exposed, but every declared permission/shell/network capability is not yet enforced at each tool boundary.
- #3166 fleet dogfood smoke and #3167 org-chart/setup UI are product/soak scopes per the coverage doc; not safe as unattended hot-path patches in this train.
- No full `cargo test -p codewhale-tui --bins --locked` run was attempted.
