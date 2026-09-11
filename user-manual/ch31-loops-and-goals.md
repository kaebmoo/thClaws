# Chapter 31 — Loops and goals (`/loop`, `/goal`)

Two commands for work that needs more than one turn.

**`/loop`** is a dumb repeater: it re-sends the same message on a timer
until you stop it. **`/goal`** is an objective the engine tracks — with
budgets, an audit discipline, and hard limits that stop a runaway.

They are most useful together: a goal supplies the *what* and the
stopping condition, a loop supplies the *keep going*. But each works
alone, and `/goal --auto` removes the need for the loop entirely.

## `/loop` — repeat something on a timer

```
❯ /loop 30s check whether the build finished and summarise any new errors
loop started (every 30s): check whether the build finished and summarise any new errors
```

The first token is an interval (`30s`, `5m`, `2h`, `1d`); everything
after it is the message that gets re-sent. Omit the interval and the
whole line becomes the body, fired every **5 minutes**:

```
❯ /loop /goal continue
loop started (self-paced (5min default)): /goal continue
```

Managing it:

| Command | What it does |
|---|---|
| `/loop <interval> <body>` | Start. Aliases: none — the interval must come first to be read as one |
| `/loop` or `/loop status` (or `list`) | Show the active loop's body |
| `/loop stop` (or `cancel` / `kill` / `off`) | Stop it |

**One loop at a time.** Starting a second while one runs prints
`loop already running — /loop stop first` rather than stacking them.

The body is sent exactly as typed, so it can be a prompt, a slash
command, or anything else you would type yourself. The loop does not
wait for the previous turn to finish before the next interval elapses —
it is a timer, not a queue.

## `/goal` — an objective the engine tracks

```
❯ /goal start "migrate every test file to the new fixture" --budget-tokens 400000
```

A goal carries an objective, optional budgets, a running count of
tokens and iterations, and a status. It persists in the session, so it
survives `/load`.

`/goal continue` is the interesting one. It doesn't just re-prompt —
it builds an **audit prompt** from the current state that asks the
model to:

- restate the objective as concrete deliverables,
- build a checklist from the prompt to the artifacts,
- inspect real evidence — files, test output, command results,
- **not** accept proxy signals as completion,
- treat uncertainty as *not achieved*.

That last pair is the whole point. A model asked "are you done?" tends
to say yes. A model asked "check the artifacts and treat uncertainty as
not achieved" goes and looks.

### The commands

| Command | What it does |
|---|---|
| `/goal start <objective> [flags]` (or `set` / `new`) | Begin. Quote the objective to include words that start with `--` |
| `/goal` or `/goal status` | One-line state |
| `/goal show` (or `info`) | Full state: budgets, tokens used, iterations, last audit |
| `/goal continue` (or `next`) | Fire one audit iteration |
| `/goal complete [reason]` (or `done`) | Mark it done yourself |
| `/goal abandon [reason]` (or `stop` / `cancel`) | Give up on it |

### Flags on `/goal start`

| Flag | Effect |
|---|---|
| `--budget-tokens N` (or `--tokens`) | Soft ceiling on tokens. At 1.0× the model is nudged to wrap up; at 1.5× the engine stops it (below) |
| `--budget-time T` (or `--time`) | Same, for wall-clock — `30m`, `2h` |
| `--auto` (or `--auto-continue`) | Keep iterating without a `/loop` wrapper (below) |
| `--require <path>` | A file that must exist before the goal may be marked complete. **Repeatable** (below) |

## Running a goal to completion

Two ways.

**With a loop** — the explicit version:

```
❯ /goal start "get the integration suite green" --budget-time 2h
❯ /loop 2m /goal continue
```

Every two minutes, one audit iteration. When the goal reaches a
terminal status the loop **stops itself**:

```
loop auto-stopped (goal complete)
```

**With `--auto`** — no loop at all:

```
❯ /goal start "get the integration suite green" --budget-time 2h --auto
```

After each turn that made tool calls, the engine queues the next
`/goal continue` immediately — no waiting for an interval. This is
usually what you want: it is faster, and it can't double-fire.

Don't combine them. `--auto` deliberately stands down while a `/loop`
is active, precisely so the two don't both queue an iteration.

## The safety rails

This is the part worth reading before you leave a goal running
unattended. There are four independent stops, and they exist because
"the model decides when it's finished" is not a safe design on its own.

### 1. Soft budget, then hard limit

Crossing your token or time budget at **1.0×** swaps in a
wrap-up prompt — the model is told it is out of budget and should
converge.

Crossing **1.5×** is the engine's business, not the model's. The goal
is forced to `blocked`, the loop aborts, and you get told why:

```
/goal continue — hard limit: token budget overrun (612000 used ≥ 1.5× 400000 budget).
Auto-blocking goal + stopping loop.
```

The 1.5× grace exists so a model that is genuinely one step from done
isn't cut off mid-sentence, while a model that has stopped converging
can't keep spending.

### 2. The iteration cap

**100 `/goal continue` firings**, whether or not you set a budget. A
real audit converges well under this; the cap only catches runaways.
It is the reason a goal with no budgets at all is still bounded.

### 3. The empty-turn guard

If a turn inside a loop produces **zero tool calls** — the model
monologued instead of doing anything — the next firing is skipped once:

```
(/goal continue suppressed: prior turn made no tool calls — model just
 monologued. Will retry next /loop firing.)
```

One firing, not permanently: the guard resets itself so a single
thoughtful turn doesn't kill the loop.

### 4. Terminal status stops everything

`complete`, `abandoned` and `blocked` are terminal. Reaching any of
them aborts an active loop and refuses further `/goal continue`. A
blocked goal is *paused*, not dead — resolve the blocker and run
`/goal continue` manually, or `/goal abandon` it.

## `--require` — a gate the model cannot talk its way past

The audit prompt asks the model to verify artifacts. `--require` makes
the engine check instead:

```
❯ /goal start "write the migration guide" \
    --require docs/migration.md \
    --require docs/migration-th.md \
    --auto
```

`MarkGoalComplete` is now **rejected** while either file is missing,
with the missing paths named. The goal stays `active`, so an `--auto`
loop keeps working rather than declaring victory.

This turns a prompt convention into a hard fact. A model can be
convinced it wrote a file; it cannot make the file exist. Use it
whenever the goal's output is a file you can name up front — it is the
single most effective thing in this chapter.

Paths are relative to the working directory, and the flag repeats.

## What the model can do to a goal

Three tools, deliberately split so that "checkpoint" and "declare
victory" are different acts:

| Tool | Effect |
|---|---|
| `RecordGoalProgress` | Mid-loop checkpoint. Status stays `active`, iterations continue. The summary is carried into later iterations as a `prior_audit` hint, so they don't re-audit from scratch |
| `MarkGoalComplete` | Terminal `complete`. Requires an `audit` summary of what was checked and the evidence. Refused if any `--require` path is missing |
| `MarkGoalBlocked` | Terminal `blocked`. Requires a `reason` — a missing key, an ambiguous spec, a decision only you can make |

The split matters. Before it, a single tool did all three, and the
cheapest path for an uncertain model was to declare completion. Now the
uncertain move is `RecordGoalProgress`, which keeps the loop running —
the tool descriptions say so explicitly: *"ending the loop on
insufficient evidence is the worst failure mode."*

## When not to use this

- **A single well-specified task.** Just ask. A goal adds bookkeeping
  and audit turns you don't need.
- **Work that fans out over many files.** That is
  [Workflows](ch25-workflows.md) — deterministic, parallel, resumable.
  A goal is one line of work that keeps going, not many lines at once.
- **Anything on a schedule rather than a duration.** `/loop` dies with
  the session. For "every weekday at 08:30", use
  [Scheduling](ch19-scheduling.md), which survives restarts and can run
  from a daemon.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `loop already running` | One loop per session | `/loop stop` first |
| Loop fires but nothing happens | Body is being suppressed by the empty-turn guard | Look for the suppression notice; the next firing retries |
| Goal went `blocked` on its own | Hard limit — 1.5× a budget, or 100 iterations | `/goal show` names the reason. Raise the budget on a fresh goal, or narrow the objective |
| Model keeps saying it's done, but it isn't | Nothing forces it to prove anything | Restart the goal with `--require <path>` for each artifact |
| `--auto` isn't firing | A `/loop` is active, or the last turn made no tool calls | `/loop stop` and let `--auto` drive |
| Goal survived a restart you didn't want | Goals persist in the session | `/goal abandon`, or start a new session |

## See also

- [Chapter 18](ch18-plan-mode.md) — plan mode, for when you want
  *ordered steps you approve* rather than an open objective. Plan mode
  has its own driver and its own retry budget.
- [Chapter 25](ch25-workflows.md) — workflows, for bulk fan-out.
- [Chapter 19](ch19-scheduling.md) — schedules, for repetition that
  outlives the session.
