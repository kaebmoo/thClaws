# `/loop` + `/goal` — iteration scheduler + audit-driven completion

Two interlocking primitives that compose into a Ralph-style overnight builder. `/loop` is the recurring-iteration scheduler (any line, any interval). `/goal` is the structured-objective + completion-audit pattern (one objective, audit-gated termination, model-callable goal tools). They compose so the canonical use is `/loop 30s /goal continue` — fire the audit prompt every 30 seconds until the model verifies completion and calls `MarkGoalComplete`.

This doc covers: the slash command surface, the `LoopState` + `GoalState` types, the broadcaster pattern, the three goal tools, how `/goal continue` becomes an agent turn (rewrite-before-match), session-scope behavior, the embedded `goal_continue.md` template, the auto-stop logic on terminal goal status, and the testing surface.

**Source modules:**
- `crates/core/src/goal_state.rs` — `GoalState`, `GoalStatus`, global state + broadcaster, `build_audit_prompt`
- `crates/core/src/tools/update_goal.rs` — `RecordGoalProgressTool` / `MarkGoalCompleteTool` / `MarkGoalBlockedTool` (model-callable hooks; authority split by intent, Phase C1)
- `crates/core/src/default_prompts/goal_continue.md` — embedded audit prompt template
- `crates/core/src/repl.rs` — `SlashCommand::{Loop, LoopStop, LoopStatus, GoalStart, GoalStatus, GoalContinue, GoalComplete, GoalAbandon, GoalShow}`, `parse_loop_subcommand`, `parse_goal_subcommand`, `parse_duration_secs`, CLI dispatch + rewrite-before-match
- `crates/core/src/shell_dispatch.rs` — GUI dispatch arms; `format_goal_status` / `format_goal_show` helpers
- `crates/core/src/shared_session.rs` — `WorkerState.active_loop`, `ActiveLoop` struct, GUI handle_line rewrite for `/goal continue`, dispatch signature now takes `input_tx`

**Cross-references:**
- [`agentic-loop.md`](agentic-loop.md) — what `/goal continue` runs (a regular agent turn against the audit prompt)
- [`built-in-tools.md`](built-in-tools.md) — the `Tool` trait and registry these three plug into
- [`commands.md`](commands.md) — slash command framework
- [`sessions.md`](sessions.md) — goal state per-session (future: persist to JSONL)

---

## 1. Concept

The pair implements the **goal-continue design pattern** — disciplined self-audit as the primary exit signal, with iteration timing handled separately.

```
USER:  /goal start "ship the auth refactor" --budget-tokens 200000
USER:  /loop 30s /goal continue

LOOP fires every 30s:
  → ShellInput::Line("/goal continue") arrives at worker
  → handle_line intercepts before slash dispatch
  → builds audit prompt from goal_state::current() + template
  → record_iteration(0) — counter bumps
  → agent.run_turn(prompt) — model reads conversation, decides next action
  → if model calls MarkGoalComplete → goal terminal
  → post-turn check sees terminal status → loop auto-stops
  → emits "loop auto-stopped (goal complete)" notice
```

If the model returns without calling any of the three, the next loop firing happens normally. `MarkGoalBlocked` also auto-stops the loop, and the user sees the blocker reason.

## 2. `/loop` slash command

| Syntax | Effect |
|---|---|
| `/loop` (or `/loop status`) | Show active loop state |
| `/loop stop` (or `cancel` / `kill` / `off`) | Stop the active loop |
| `/loop <interval> <body>` | Fire `<body>` every `<interval>` (e.g. `/loop 30s /goal continue`) |
| `/loop <body>` | Self-paced (default 5 min interval); `body` doesn't start with a duration token |

### Interval grammar

`parse_duration_secs` accepts `Ns` / `Nm` / `Nh` / `Nd` (case-insensitive on the unit). If the first token of the args doesn't parse as a duration, the **whole input** is treated as the body and the loop runs at the default 5-minute cadence.

### State

```rust
pub struct ActiveLoop {
    pub interval_secs: Option<u64>,   // None = self-paced default
    pub body: String,                  // line fired each iteration
    pub started_at: u64,
    pub iterations_fired: u64,
    pub abort: tokio::task::AbortHandle, // stop handle
}

// In WorkerState:
pub active_loop: Option<ActiveLoop>,
```

Single active loop per session. Starting a new one while one's running errors with "loop already running — `/loop stop` first."

### Implementation

GUI path (in shell_dispatch.rs `dispatch`):
1. Validate no existing loop
2. Resolve interval (`interval_secs.unwrap_or(300)` for self-paced default)
3. Spawn `tokio::spawn(async move { loop { sleep(interval); input_tx.send(body) } })`
4. Store `AbortHandle` in `WorkerState.active_loop`

CLI path (in repl.rs run_repl): same shape but uses a tokio mpsc channel (`cli_input_rx`) that the readline `select!` arm pulls from. The body is rendered into a "[loop fired]" notice + becomes the next `line` for the iteration.

### Auto-stop conditions

- **Manual** `/loop stop`
- **Goal terminal**: post-`/goal continue` turn, if goal state is Complete/Abandoned/Blocked, the loop is aborted
- **Channel closed**: worker shutdown — task exits silently
- **Process exit**: tokio runtime drop kills all spawned tasks

## 3. `/goal` slash command

| Syntax | Effect |
|---|---|
| `/goal` (or `/goal status`) | Short status line |
| `/goal show` | Full goal contents (objective, budgets, last audit, etc.) |
| `/goal start "<objective>" [--budget-tokens N] [--budget-time T]` | Start a new goal. Registers the three goal tools |
| `/goal continue` (or `next`) | Fire one audit-prompt iteration. Agent turn — composable with `/loop` |
| `/goal complete [reason]` | Manual override: mark complete, auto-stop loop |
| `/goal abandon [reason]` | Manual stop with reason, auto-stop loop |

`<objective>` can be quoted (`"build a REST API"`) for multi-word; unquoted strings consume words up to the first `--flag`.

`--budget-tokens` accepts a u64. `--budget-time` accepts a duration (`30m`, `2h`, etc.).

### State

```rust
pub struct GoalState {
    pub objective: String,
    pub started_at: u64,                  // Unix timestamp
    pub budget_tokens: Option<u64>,
    pub budget_time_secs: Option<u64>,
    pub tokens_used: u64,                 // running counter (approximate)
    pub iterations_done: u64,
    pub status: GoalStatus,               // Active | Complete | Abandoned | Blocked
    pub last_audit: Option<String>,       // from RecordGoalProgress / MarkGoalComplete
    pub last_message: Option<String>,     // blocker reason, completion summary
    pub completed_at: Option<u64>,
}
```

Per-session, kept in a global `OnceLock<Arc<Mutex<Option<GoalState>>>>` (mirrors `plan_state`). Read via `goal_state::current()`, mutated via `goal_state::apply(|g| { ...; true })`. Future: persist to session JSONL as `{"type": "goal_state", ...}` events for `/load` resumption.

### `GoalStatus`

| Variant | Meaning | Loop reaction |
|---|---|---|
| `Active` | Goal in progress | Loop continues |
| `Complete` | Audit passed; objective achieved | Loop auto-stops |
| `Blocked` | Model needs user input | Loop auto-stops |
| `Abandoned` | User manually stopped | Loop auto-stops |

`is_terminal()` returns true for the latter three.

## 4. The audit prompt

`/goal continue` builds the prompt by filling [`default_prompts/goal_continue.md`](../thclaws/crates/core/src/default_prompts/goal_continue.md) with the current goal state:

| Variable | Source |
|---|---|
| `{{ objective }}` | `goal.objective` (wrapped in `<untrusted_objective>` tags) |
| `{{ time_used_seconds }}` | `goal.time_used_secs()` |
| `{{ tokens_used }}` | `goal.tokens_used` |
| `{{ token_budget }}` | `goal.budget_tokens` or "(unlimited)" |
| `{{ remaining_tokens }}` | `tokens_remaining()` or "(unlimited)" |
| `{{ iterations_done }}` | `goal.iterations_done` |
| `{{ prior_audit }}` | `goal.last_audit` or "(none — first iteration)" |

The template bakes in the audit discipline:
- Restate objective as concrete deliverables
- Build a prompt-to-artifact checklist
- Inspect concrete evidence (files, tests, output)
- Don't accept proxy signals as completion
- Treat uncertainty as not achieved
- Distinguish "stopping" from "complete"

The model is instructed to call `MarkGoalComplete` only after auditing every requirement against concrete evidence — and when the goal declared `--require` paths, the engine checks them too.

## 5. The three goal tools

`UpdateGoal` was **split into three tools** in Phase C1
(`a8946811`) — `tools/update_goal.rs`. Authority is separated by
intent: one non-terminal checkpoint, two terminal transitions. The
point is that a model cannot slip into "mark complete to escape the
loop" by reusing the routine progress call, because completing is a
different tool with a different required argument.

| Tool | Effect | Required arg |
|---|---|---|
| `RecordGoalProgress` | Status stays **Active**; stashes the audit, carried into the next iteration as `prior_audit` | `audit` |
| `MarkGoalComplete` | Status → **Complete**, `completed_at` stamped, loop auto-stops post-turn | `audit` |
| `MarkGoalBlocked` | Status → **Blocked**, `last_message` set, loop auto-stops post-turn | `reason` (`audit` optional — what was done before the blocker) |

All three mutate global goal state through `goal_state::apply`, which
fires the broadcaster: the worker persists a snapshot to the session
JSONL, auto-stops the loop on a terminal status, and refreshes the goal
sidebar.

**Completion is engine-gated, not just model-asserted.** When the goal
was started with `/goal start --require <paths>`, `MarkGoalComplete`
checks `missing_required_paths()` first. If any declared artifact is
absent the goal **stays Active** and the tool returns the list of what
is missing — the model cannot declare victory over a file it never
wrote (`53ee1f82`).

`requires_approval = false` for all three: each call is a small
in-memory state change, not a disk write. The worker validates that a
goal is actually active before the call takes effect; with no active
goal the call returns an explanatory error rather than failing — the
same shape as `KmsRead` against an unknown KMS.

Registered at `/goal start`, not in the default registry — the model
only sees them while a goal is active.

## 6. Composition: `/loop /goal continue`

The full Ralph-style pattern:

```
USER:  /goal start "complete the auth refactor" --budget-tokens 200000 --budget-time 1h
       → goal state initialized, the three goal tools registered
USER:  /loop 60s /goal continue
       → tokio task spawned; fires `/goal continue` every 60s
EVERY 60s:
       → input_tx.send(ShellInput::Line("/goal continue"))
       → handle_line intercepts (rewrite-before-match)
       → goal_state::current() → Some(GoalState { status: Active, ... })
       → build_audit_prompt(g) — fills template
       → record_iteration(0)
       → agent.run_turn(audit_prompt)
       → model: reads chat history, picks next action, possibly calls
                Bash/Read/Edit/Write/Grep/etc., possibly records progress
       → post-turn check: goal_state.status terminal? → abort loop
       → otherwise next 60s firing happens
EVENTUALLY:
       → model calls MarkGoalComplete { audit: "..." }
       → goal_state.status = Complete
       → post-turn: loop aborted, "loop auto-stopped (goal complete)" emitted
```

User can interrupt anytime via `/loop stop` or `/goal abandon`.

## 7. Rewrite-before-match pattern

`/goal continue` doesn't go through `shell_dispatch::dispatch` — it's intercepted **before** the slash match in `handle_line` (GUI) and the equivalent point in `run_repl` (CLI), because it needs to:
1. Read goal state
2. Build a prompt
3. Run an agent turn (`drive_turn_stream`)
4. Check goal state post-turn for auto-stop

`shell_dispatch::dispatch` has a defensive arm that surfaces a clear error if `/goal continue` ever lands there directly (which shouldn't happen in normal flow).

This mirrors the `/kms ingest <name> $` pattern from M6.28 — slash commands that need to run agent turns intercept at handle_line, not in dispatch.

## 8. CLI vs GUI differences

| Aspect | CLI (`run_repl`) | GUI (`handle_line`) |
|---|---|---|
| Loop body delivery | `tokio::sync::mpsc::unbounded_channel<String>` consumed by readline `select!` arm | `std::sync::mpsc::Sender<ShellInput>` (the worker's input queue) |
| Active loop state | Local `active_loop_handle: Option<AbortHandle>` + `active_loop_body: Option<String>` | `WorkerState.active_loop: Option<ActiveLoop>` |
| Auto-stop on goal terminal | Inline check after each turn in the readline loop | Post-turn check in `handle_line` after `drive_turn_stream` |

## 9. Testing

| Module | Tests | Coverage |
|---|---|---|
| `goal_state::tests` | 8 | current/set/apply round-trip, no-goal apply returns false, record_iteration counters, status terminality, tokens_remaining, build_audit_prompt template substitution |
| `tools::update_goal::tests` | 6 | complete/blocked/progress status updates, audit + reason recording, no-active-goal error, invalid-status error, approval posture |
| `repl::tests` | 8 | parse_slash_loop_status, parse_slash_loop_stop, parse_slash_loop_with_interval, parse_slash_loop_self_paced, parse_duration_secs_units, parse_slash_goal_lifecycle, parse_slash_goal_start_with_budgets, parse_slash_goal_start_missing_objective_errors |

824 lib tests pass (was 801 after M6.28 follow-up). Tests using the global goal state serialize via a per-test mutex.

No GUI E2E tests — the spawn-task / channel plumbing is verified by build + manual smoke test.

## 10. Known limitations / future work

- **Goal state not persisted to session JSONL yet** — closing the session loses the goal. Adding a `goal_state` JSONL event mirroring the `plan_snapshot` pattern is a small follow-up.
- **`tokens_used` is approximate** — currently always 0 because the agent's per-turn usage isn't yet plumbed back into `record_iteration`. Easy follow-up: thread agent.usage through `drive_turn_stream`.
- **No model-controlled pacing** — self-paced loops use a fixed 5min default. A `RescheduleLoop` tool would let the model decide "fire again in N seconds" based on what it just did.
- **Single global goal** — can't track parallel objectives. Same posture as `plan_state`. Could be a Vec in a future sprint.
- **Loop body doesn't validate** — if you `/loop 30s /nonexistent` the loop happily fires `/nonexistent` every 30s. The `parse_slash` returning `Unknown` would print a warning each time. Consider validation at start.
- **Stalled-loop detector not built** — if Ralph wedges (model returns same answer N times), no automatic notification. Pattern exists in M4.4 plan-stalled but not wired here.

## 11. Sprint chronology

- **M6.29** (`dev-log/145`) — initial implementation of `/loop`, `/goal`, the then-single `UpdateGoal` tool, audit-prompt template, integration auto-stop. Split into three tools in Phase C1 (`a8946811`).
