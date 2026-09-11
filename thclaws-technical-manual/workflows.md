# Dynamic Workflows

Code-driven subagent fan-out. The model **authors** a JavaScript
orchestration script from a plain-language goal; a Boa interpreter
**executes** that script deterministically; workers run as stateless
subagents with fresh context. The script is the plan, and it is
inspectable, editable and re-runnable before a single worker spawns.

This is the fourth orchestration tier, and the only one where the
control flow is data rather than model judgement:

| Tier | Who decides the next step | Doc |
|---|---|---|
| `Task` tool | The model, mid-turn | [`subagent.md`](subagent.md) |
| `/agent <name>` | The user, concurrently with the main turn | [`side-channel.md`](side-channel.md) |
| Agent Teams | Teammates, through filesystem mailboxes | [`agent-team.md`](agent-team.md) |
| **Workflows** | **A script, authored once and then fixed** | this doc |

The distinction matters when a fan-out has to be *repeatable*. A model
asked to "review these 30 files" may review 28 of them; a script that
loops over 30 entries reviews 30.

Source: `crates/core/src/workflow/` — `runtime.rs` (the sandbox and host
bindings), `script.rs` (the author phase), `approval.rs` (the review
gate), `state.rs` (the run log), `inspect.rs` (`/workflow inspect`),
`headless.rs` (`thclaws --workflow`).

## 1. The three phases

```
  /workflow run <goal>
        │
        ▼
  ┌──────────────┐   the active provider writes JS against the
  │  1. Author   │   WORKFLOW_AUTHOR system prompt. No conversation
  └──────┬───────┘   history from the calling session is included.
         ▼
  ┌──────────────┐   the user sees the script and picks
  │  2. Review   │   Approve / Cancel / Rework("do X instead")
  └──────┬───────┘   Rework loops back to author, revision++
         ▼
  ┌──────────────┐   Boa runs the script; thclaws.subagent calls
  │  3. Execute  │   spawn real workers; every event lands in
  └──────────────┘   state.jsonl as it happens
```

**Author** (`script.rs::author`) sends the API spec as the system prompt
and the goal as the only user message — deliberately no history, so a
long conversation cannot leak into an orchestration script.

**Review** (`approval.rs`) mirrors `permissions::GuiApprover`: the
dispatcher emits a `ViewEvent::WorkflowReviewRequest`, registers a
oneshot keyed by workflow id, and awaits. `Rework(note)` re-authors with
the note attached and increments a revision counter so the UI can label
"Revision 2". A dropped receiver resolves as `Cancel`, so session
shutdown needs no separate timeout path.

**Execute** runs the script in a Boa context on a blocking thread.

## 2. The sandbox

Boa, not Node. There is no filesystem, no network, no process. Two
globals are actively removed rather than merely absent:

```rust
global.delete_property_or_throw(js_string!("eval"), ctx)?;
global.delete_property_or_throw(js_string!("Function"), ctx)?;
```

so an authored script cannot construct new code at runtime — what you
reviewed is what runs. `console` is stripped too; `thclaws.log()` is the
blessed narration channel.

Everything the script can reach lives on one read-only global:

| Binding | Purpose |
|---|---|
| `thclaws.subagent(spec)` | Spawn one worker and return its output |
| `thclaws.parallel(specs)` | Spawn many **concurrently** — the only real fan-out primitive |
| `thclaws.pollUntil(fn, opts)` | Re-run a check until it passes or a bound is hit |
| `thclaws.log(msg)` | One narrator line to stdout, and a chat indicator under the GUI |
| `thclaws.include(path)` | Pull in a companion script |

`thclaws` is registered with `Attribute::READONLY`, so a script cannot
reassign the object to shadow the host functions.

### `subagent(spec)`

The spec carries a `prompt`, and optionally:

- `agent: "name"` — use a subagent definition from `.thclaws/agents/<name>.md`, the same lookup the model-driven `Task` tool uses.
- `schema` — a JSON schema the worker's output must satisfy. When absent, the named agent's declared `output_schema` is used, so the schema can live with the agent definition instead of being repeated in every script.
- `caps` — see below.

Calls route through the real `Task` primitive, so a workflow worker is
an ordinary subagent: fresh context, its own registry, no access to the
parent's conversation.

### `parallel(specs)`

Concurrency is capped by a semaphore, default **8**, overridable with
`THCLAWS_WORKFLOW_PARALLELISM` and clamped to 1–32. The default exists
because an uncapped fan-out will happily open thirty streams against a
gateway that wanted eight; raise it when the gateway's budget allows.

`parallel` is the *only* primitive that runs work concurrently — a
`for` loop over `subagent()` is sequential, which is occasionally what
you want and usually not.

## 3. Capabilities

A worker inherits nothing by default. `caps` opts one worker into one
narrow thing:

```js
thclaws.subagent({ prompt: "…", caps: { kms: { write: ["notes"] } } })
```

`WorkerCaps` currently carries `kms_write` — the set of KMS names the
worker may write to. A `KmsWrite` against a KMS not in that set is
denied at the host boundary, not left to the worker's judgement. The
grant is checked from a task-local first and a thread-local second, so
interleaved futures in a `parallel` stage cannot read each other's
grants.

This is why a workflow that writes research into a KMS must say so in
the script — and why reading the script tells you what the run can
touch.

## 4. Retries

A single dropped SSE stream during a thirty-worker fan-out used to kill
the whole run. Workers now retry up to `MAX_ATTEMPTS = 3`, but only for
failures classified as **transient** by
`workflow::is_transient_error` — dropped streams, network blips,
upstream overload. Deterministic failures (auth, bad request, schema
mismatch, empty output) fail immediately, because retrying them just
burns tokens to reach the same error.

Every retry is logged as its own `worker_retry` event carrying the
prior error, so a run that succeeded on attempt three does not look
like a run that succeeded on attempt one.

## 5. The run log

`<cwd>/.thclaws/state/workflows/<id>/state.jsonl` — append-only, one
JSON object per line, flushed after every write so a Ctrl-C leaves a
readable file rather than half a record. Same convention as session
JSONL, and deliberately `cat`-friendly.

| `kind` | Emitted when |
|---|---|
| `start` | Run begins — carries `script_sha` and `script_chars` |
| `worker_start` | A worker is dispatched — carries its `prompt` |
| `worker_caps` | The worker's granted capabilities, recorded before it runs |
| `worker_done` | Worker returned — carries `output` |
| `worker_retry` | Transient failure — carries `attempt` and `prior_error` |
| `worker_error` | Worker failed terminally |
| `done` / `error` | Run finished |

`script_sha` is what makes a run reproducible: the log records exactly
which script produced these workers.

## 6. Resume

`/workflow resume <id>` replays the completed workers from
`state.jsonl` instead of re-running them. The replay cache is
**prompt-matched**: a cached output is only reused when the prompt at
that position is identical. If you edited the script and the prompts
moved, the cache falls through and the worker runs for real — so
resuming an edited script does the right thing rather than silently
serving stale output for a call that no longer exists.

## 7. Entry points

| Surface | Path |
|---|---|
| `/workflow run <goal>` | Author → review → execute. The normal path |
| `/workflow exec <file.js>` | Skip authoring, run a script you wrote |
| `/workflow list` | Past runs |
| `/workflow inspect <id>` | Replay one run's log — `inspect.rs` |
| `/workflow rm <id>` | Delete a run's state |
| `/workflow resume <id>` | Continue from the last completed worker |
| `thclaws --workflow <file.js>` | Headless, no author or review phase — the script is pre-vetted by the operator (`headless.rs`). `--resume` loads the completed-worker cache |
| `WorkflowRun` tool | Model-callable, `tools/workflow_run.rs` — lets an agent start a workflow mid-turn |

The headless entry mirrors `telegram::headless` for environment
construction (provider, system prompt, KMS + Memory tools) and adds
`SubAgentTool` so `thclaws.subagent` routes through a real
`ProductionAgentFactory`.

## 8. What to watch for

- **`parallel` is the only concurrency.** A `for` loop of `subagent()` calls is serial. This has surprised people whose "parallel" workflow took thirty minutes.
- **The default cap is 8, not unbounded.** A gateway with a tighter budget needs `THCLAWS_WORKFLOW_PARALLELISM` lowered, not the script rewritten.
- **Caps are per-call, not per-run.** Granting `kms.write` to one worker does not grant it to the next one in the same script.
- **The author phase sees no history.** If the goal depends on context from the conversation, that context has to be in the goal string.
- **Resume matches on prompt text.** Reordering calls in a script invalidates the cache for everything after the first change.

## See also

- [`subagent.md`](subagent.md) — the `Task` primitive workers actually run on
- [`side-channel.md`](side-channel.md) — the user-driven concurrent tier
- [`agent-team.md`](agent-team.md) — the multi-process tier
- [`user-manual/ch25`](../user-manual/ch25-workflows.md) — the user-facing guide
