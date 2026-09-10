# Chapter 17 — Agent Teams

Agent Teams let you run **multiple thClaws agents in parallel**,
coordinating through a filesystem-based mailbox and task queue. Useful
when work genuinely fans out: backend + frontend at the same time,
one agent writing tests while another implements, etc.

Teams are **opt-in** — they spin up extra processes and burn tokens
fast.

**From the GUI.** Click the gear icon → the *Workspace* section has
an *Agent Teams* row with an on/off pill. Click to toggle. The change
writes `teamEnabled: true` to `.thclaws/settings.json` and you'll see
a yellow "Restart the app for this to take effect" notice — team
tools are registered at session spawn, so the running shared session
needs a respawn to pick them up.

**From the CLI or by hand:**

```json
// .thclaws/settings.json
{ "teamEnabled": true }
```

With `teamEnabled: false` (the default), no team tools are registered
and no inbox poller runs. The Team tab in the GUI stays visible either
way — it shows an empty-state pointer ("No team agents running — ask
the agent to create a team") so you can always see when a team starts
up. Sub-agents (Chapter 15) are unaffected by this flag.

> ⚠ **Provider constraint: `agent/*` models cannot use thClaws teams.**
> The `agent/*` provider ([Chapter 6](ch06-providers-models-api-keys.md))
> shells out to your local `claude` CLI as a subprocess. That subprocess
> uses Claude Code's own built-in toolset (`Agent`, `Bash`, `Edit`,
> `Read`, `ScheduleWakeup`, `Skill`, `ToolSearch`, `Write`) and does
> not see thClaws's tool registry — so even with `teamEnabled: true`,
> our `TeamCreate` / `SpawnTeammate` / etc. are unreachable from the
> model. To use thClaws teams, switch to any non-`agent/*` provider
> (`claude-opus-5`, `claude-sonnet-5`, `gpt-5`, …) via
> `/model` or `/provider`. The system prompt grounds the model to
> tell you this explicitly if you ask for a team while on `agent/*` —
> rather than silently calling Claude Code's separate built-in
> TeamCreate that writes to `~/.claude/teams/` (invisible to thClaws).

With `agent/*` and `teamEnabled: false`, the same grounding tells the
model to NOT fall back to Claude Code's `TeamCreate` / `Agent` /
`TodoWrite` / `AskUserQuestion` / `ToolSearch` built-ins — otherwise
the model would happily fabricate a "team created" response with
nothing actually written to `.thclaws/state/team/`. See dev-log 078 for
the audit that motivated this.

## Anatomy

```
.thclaws/state/team/
├── config.json                  team config (members, lead)
├── inboxes/{agent}.json         per-agent inbox (JSON array)
├── tasks/{id}.json              task queue entries
├── tasks/_hwm                   high-water mark for task IDs
├── agents/{agent}/status.json   heartbeat + current task
└── agents/{agent}/output.log    teammate stdout/stderr (background spawns)
```

Everything is a file — no DB, no broker. `fs2` advisory locking keeps
inbox writes atomic across processes.

## Team tools

All added to the agent's registry when `teamEnabled: true`:

| Tool | Purpose |
|---|---|
| `TeamCreate` | Create a team with named agents |
| `SpawnTeammate` | Launch a teammate process (tmux pane or background) |
| `SendMessage` | Write to a teammate's inbox |
| `CheckInbox` | Read unread messages, mark as read |
| `TeamStatus` | Agents + task queue summary |
| `TeamTaskCreate` | Add a task (with optional dependencies) |
| `TeamTaskList` | List tasks by status |
| `TeamTaskClaim` | Claim a pending unblocked task (teammate) |
| `TeamTaskComplete` | Mark done + notify lead |
| `TeamMerge` | Merge a teammate's worktree branch back into main |

## Spinning up a team

Typical lead prompt:

```
❯ Create a team with two members: "backend" (for the API) and
  "frontend" (for the React app). Use backend.md and frontend.md
  definitions under .thclaws/agents/. Spawn both now.
```

The lead calls `TeamCreate` then `SpawnTeammate` twice. Teammate
processes boot as `thclaws --team-agent backend --team-dir <abs path>`
(and similar), each with its own inbox and status file. Those flags
set `THCLAWS_TEAM_AGENT` and `THCLAWS_TEAM_DIR` in the child, which is
what everything downstream — the role guards, the sandbox root, the
inbox poller — keys off.

**Agent names become git identifiers**, so they're validated: 1–64
characters of letters, digits, `_` or `-`, starting with a letter,
digit or `_`. A name is also the branch (`team/<name>`) and the
worktree directory (`.worktrees/<name>`), which is why `my team!` or a
name with a slash is rejected up front rather than failing later at
`git worktree add`.

**`TeamCreate` also changes the lead's own role.** Its result tells the
model it is now a coordinator: delegate through `SendMessage` and
`TeamTaskCreate`, use `Read`/`Glob`/`Grep` only to review, and don't
build things itself. That is reinforced at the tool level by the role
guards below — the prompt asks, the guards enforce.

## Running style

`SpawnTeammate` picks one of three modes, in this order:

| Situation | What happens |
|---|---|
| Already inside a tmux session | Each teammate opens in a split pane of the current session, laid out tiled |
| tmux installed, but you're not in a session | A detached session named `thclaws-team` is created; further teammates split into it |
| No tmux at all | Each teammate runs as a plain background process, with stdout and stderr redirected to `agents/{name}/output.log` |

The third mode is fully supported — the GUI Team tab reads that log —
but you lose the ability to attach a terminal and type at a teammate
directly.

Where there is a tmux session, attach with `/team`:

```
❯ /team
attaching to tmux session 'thclaws-team'...
(press Ctrl+B then D to detach back here)
```

With no session to attach to, `/team` prints the roster instead:

```
❯ /team
Team agents (no tmux session):
  backend — working (task: t1)
  frontend — idle (task: -)
```

If tmux isn't installed at all it says so and points at
`brew install tmux`. `TeamStatus` works regardless — it reads the
status files, not tmux.

**Spawning is verified, not fire-and-forget.** `SpawnTeammate` waits
for the new teammate to write its own status file (leaving the
`spawning` placeholder), which normally takes under a second. A
background process that dies on boot is reported with the tail of its
`output.log` rather than silently counting as started.

Each pane is a full teammate REPL. You can talk directly to one:

```
❯ (on lead) send to frontend: "the /users endpoint now returns a new
  `displayName` field — update the profile page"
```

That becomes a `SendMessage` into frontend's inbox. The frontend
teammate picks it up on its next poll (1s interval), works on it,
and reports back via `SendMessage` to the lead.

## Task queue

Instead of direct messaging, you can post tasks:

```
TeamTaskCreate(
  subject: "Integration tests for /orders",
  description: "Write integration tests for the /orders endpoints, \
                covering the 400 and 409 paths",
  owner: "backend",
  blocked_by: ["1", "2"]
)
```

| Field | Required | Meaning |
|---|---|---|
| `subject` | yes | Short title, what shows in `TeamTaskList` |
| `description` | yes | The actual instructions the claiming teammate reads |
| `owner` | no | Reserve the task for one teammate — only that name can claim it. Omit for first-come-first-served |
| `blocked_by` | no | Task IDs that must complete first |

**You don't choose the id.** Task IDs are assigned from a
high-water-mark file (`tasks/_hwm`) under a lock, so two teammates
posting at once can't collide. Read the id back from the tool's result
before referencing it in a later `blocked_by`.

A misspelt `owner` is rejected against the team config rather than
accepted — otherwise the task would sit unclaimable forever, waiting
for a teammate that doesn't exist.

Teammates auto-claim pending unblocked tasks when idle (no inbox
messages, no in-flight task). A task with `blocked_by` becomes
claimable only once every one of those tasks is `completed`. There are
three states in all: `pending`, `in_progress`, `completed`.

Workflow:

1. Lead posts tasks `1`, `2`, `3`, with `3` blocked by `1` and `2`.
2. `backend` and `frontend` each claim whatever is claimable.
3. Done → `TeamTaskComplete` fires an `idle_notification` at the lead.
4. When `1` and `2` are both complete, `3` unblocks and whoever is idle
   picks it up.

## Worktree isolation and the team filesystem sandbox

An agent def can set `isolation: worktree`:

```markdown
---
name: backend
model: claude-sonnet-5
tools: Read, Write, Edit, Bash, Glob, Grep
isolation: worktree
---

You own the backend services. Work in your own git worktree so you
don't collide with the frontend teammate.
```

You can also set it **declaratively on `TeamCreate`**, per member, which
is the preferred route for an ad-hoc team that has no agent-def files:

```
TeamCreate(
  name: "shopflow",
  agents: [
    { name: "backend",  role: "API",   isolation: "worktree" },
    { name: "frontend", role: "React", isolation: "worktree" },
    { name: "qa",       role: "tests" }
  ]
)
```

Either way, **never write `git worktree add …` into a teammate's
prompt.** Isolation is a setting, not a shell command — a prompt that
tells the teammate to run it usually lands the worktree somewhere
outside `.worktrees/`, and `TeamCreate` will warn you if it spots that
string in a prompt.

On spawn, thClaws creates `<workspace>/.worktrees/backend` on branch
`team/backend` and runs that teammate process with `cwd =
<workspace>/.worktrees/backend/`. Changes written to relative paths stay
on that teammate's branch until the lead calls `TeamMerge`:

```
TeamMerge(only: ["backend"])
```

That runs `git merge team/backend` into the lead's current branch
(usually `main`). The parameters are:

| Parameter | Meaning |
|---|---|
| `into` | Target branch. Default: the repo's current branch |
| `only` | Allow-list of teammate names. Omitted → every `team/*` branch with commits ahead of the target |
| `dry_run` | Report what would be merged, merge nothing. Default: `false` |
| `cleanup` | After a successful merge, remove `.worktrees/<name>` and delete the merged branch. Default: `false` |

Run `dry_run: true` first to see whether there is anything ahead. If a
teammate shows zero commits, ping it to commit inside its own worktree
before merging — work that is only in its working tree isn't on the
branch yet. The tool reports commit counts and conflicts rather than
failing silently.

If `<workspace>` is not a git repo yet when the first worktree teammate
spawns, thClaws runs `git init` plus an empty initial commit for you,
so there's nothing to pre-initialise.

### How a teammate's sandbox differs from a standalone session

Read the standalone sandbox rules in
[Chapter 5](ch05-permissions.md#sandbox-filesystem) first. Solo,
the sandbox root is the cwd the session opened in.

For a teammate in a team, the sandbox root is **always the lead's
workspace**, not the teammate's own cwd. `SpawnTeammate` exports
`THCLAWS_PROJECT_ROOT` to the teammate process, and `Sandbox::init()`
reads that env var first, falling back to `cwd` only for a solo
session. So every teammate may write anywhere under the workspace
(except `.thclaws/`), no matter which folder it was placed in.

The **cwd** is then pointed at whatever folder suits the job:

- `isolation: worktree` → cwd = `<workspace>/.worktrees/<name>/`
- no isolation → cwd = `<workspace>`, same as the lead

The two settings answer different questions: cwd decides where a
relative path resolves; the sandbox root decides how far write
permission reaches.

### Two classes of file a teammate can write

| Path form | Where it lands | Who sees it, when |
|---|---|---|
| relative (`src/server.ts`) from a worktree | `<workspace>/.worktrees/backend/src/server.ts`, on branch `team/backend` | others see it after `TeamMerge` |
| absolute (`<workspace>/docs/api-spec.md`) from a worktree | `<workspace>/docs/api-spec.md`, on the workspace tree's `main` | immediately, no merge |
| relative (`tests/api.test.ts`) from a non-isolated teammate | `<workspace>/tests/api.test.ts` on `main` | immediately |

The pattern that fell out of building the ShopFlow team:

- **Shared contract** (API spec, shared TS types) — have backend write
  it to an absolute workspace path as soon as it exists, so frontend
  and qa can read it without waiting for a merge.
- **Implementation** (handlers, models, server) — relative paths inside
  the worktree; the lead merges when it's ready.
- **Tests** — qa runs non-isolated and writes straight into the
  workspace tree, running them once the implementation has merged.

### Specially denied paths

On top of the standalone rules (`..` escapes, symlink escapes, anything
outside the root):

- No teammate may write `<workspace>/.thclaws/` — use the team tools
  instead.
- Cross-worktree writes (backend writing
  `<workspace>/.worktrees/frontend/…`) are **not** blocked. That is
  prompt-design's job, matching the Claude Code reference
  implementation, which also declines to block it. If you want a
  harness-level guard, add a hook in `.thclaws/settings.json`.

### Why this model

The obvious alternative — "sandbox = the teammate's cwd", so a worktree
teammate can only write inside its own worktree — means every shared
artefact has to go through a merge before anyone else can read it. That
adds round trips for no benefit and serialises a team you spun up to run
in parallel.

The current model (sandbox = workspace, cwd = worktree) matches how
people actually use git worktrees: one shell at the repo root for shared
work, `cd` into a worktree only for branch-specific edits. It is also
what the Claude Code reference implementation does
(`getOriginalCwd()` in `utils/permissions/filesystem.ts`).

## Plan Approval (convention)

If your prompt to the lead mentions "Plan Approval", "with plan
approval", or similar wording, the system reads it as a
**lead↔teammate convention** — NOT a request to ask the human user:

1. Each teammate, before starting non-trivial work, sends a brief plan (1–3 lines: what they'll do, what they'll touch) to the lead via SendMessage.
2. Lead reviews and replies "approved, proceed" or "revise: …".
3. Teammate waits for the ack, then executes.

**The lead is the approver — never the user**, even when a human is watching. The mode only activates when the user prompt explicitly mentions it; otherwise teammates execute work directly so default behavior is preserved. Defined in `default_prompts/lead.md` and `default_prompts/agent_team.md`.

## Role guards (lead vs teammate)

To stop an LLM lead from accidentally wiping a teammate's files (e.g.
the actual `rm -rf tests/` we observed in a test run), BashTool /
Write / Edit have hard guards:

**Lead — refused regardless of `--accept-all`:**

| Command | Why blocked |
|---|---|
| `git reset --hard <ref>` | discards committed work |
| `git clean -f` / `-d` | deletes untracked files |
| `git push --force` / `git rebase` | rewrites shared history |
| `git worktree remove` / `prune` | kills teammate's process + worktree |
| `git checkout -- <path>` / `git checkout .` / `git restore --worktree` / `git restore .` | discards teammate's uncommitted work |
| `git merge --abort` | tears down a merge instead of delegating |
| `rm -rf` / `-fr` / `-r` | destructive removal |
| `Write` / `Edit` (any path) | lead is a coordinator, not the author |

`git push -f` counts the same as `git push --force`. The list matches on
the **lowercased command text**, so case games don't get through.

**Obfuscation is refused, not decoded.** A destructive command the lead
builds through `$VAR`, `$(…)`, backticks, `eval` or brace expansion is
rejected outright — the guard can't verify what such a string will
expand to, so it declines rather than guessing. Run a plain literal
command, or hand the destructive step to the teammate who owns it.

**Write/Edit exception:** when a git merge is in progress AND the target file currently contains `<<<<<<<` markers, the lead may write the resolved version. Once it commits the merge, `MERGE_HEAD` disappears and the block snaps back on automatically.

**Teammate — refused:**

| Command | Why blocked |
|---|---|
| `git reset --hard <branch-name>` (e.g. `main`, `origin/main`, `team/backend`) | resets your branch tip to a different branch — discards your own commits |

Still allowed (legitimate same-branch recovery): `HEAD~N`, `HEAD@{N}`, `HEAD^`, hex SHAs, `tags/...`.

When a guard fires, the tool returns an error explaining what's blocked and what to do instead (e.g. "delegate to a teammate via SendMessage" or "use HEAD~N rather than `main`"). Well-trained models redirect rather than retry.

### Editor stubs for teammates

SpawnTeammate sets `EDITOR=true VISUAL=true GIT_EDITOR=true GIT_SEQUENCE_EDITOR=true` on every teammate process.

So commands that would open an editor (`git commit -e`, `git commit` with no `-m`, `git rebase -i`) **don't hang** waiting for human input via `/dev/tty` — the `true` builtin exits 0 immediately, and git uses whatever message was already provided via `-F`/`-t` or commits empty per default. Prevents `vi` or `nano` from stalling the entire team mid-run.

## Protocol messages

Standard message types teammates and lead exchange:

| Type | From → To | Meaning |
|---|---|---|
| `idle_notification` | teammate → lead | "I finished task X" — carries `idle_reason`, the task id, its final status and a summary |
| `shutdown_request` | lead → teammate | "Stop and exit cleanly" — sent to every member when the lead exits |
| `shutdown_approved` | teammate → lead | "Nothing in flight; I'm stopping." The teammate writes `stopped` and exits |
| `shutdown_rejected` | teammate → lead | "Still have unfinished tasks" — the teammate keeps polling |
| `abort_turn` | lead → teammate | Cancel the current turn cooperatively — the only way to interrupt a headless teammate, which never receives Ctrl+C |
| `user` | user → teammate | Free-form text (via `send to <agent>: …`) |

`idle_notification` is not only a "done" signal. Its `idle_reason`
field distinguishes the outcomes the lead has to react to differently:

| `idle_reason` | What it means |
|---|---|
| `available` | Clean finish, ready for the next task |
| `interrupted` | The turn was cut short |
| `failed` | The turn failed — a provider or config error, not a code problem |
| `blocked` | Gave up mid-task (hit `max_iterations` or a time budget); the task is left for the lead to re-drive |

**Shutdown is a negotiation, not a kill.** On exit the lead sends
`shutdown_request` to every member and waits ~1.2 s. A teammate with a
queued message or an in-progress task answers `shutdown_rejected` and
carries on. Only then do the hard fallbacks run — the child handles the
lead owns, and (on Unix) a `pkill` on the teammates' `--team-dir`
argument. So closing the lead does not, by itself, discard a teammate's
work in flight.

## Monitoring in the GUI

The Team tab shows one pane per teammate plus a `lead` pane mirroring
the main terminal. ANSI colours are translated to HTML: green for LLM
text, cyan for prompts and inbox messages, dim for tool starts and
token lines, yellow for errors or hit-max-iterations.

Status comes from each teammate's own `status.json` — no false crash
flagging based on missing heartbeats. You'll see `spawning` (written by
the lead, before the teammate has booted), then `idle`, `working`, and
finally `stopped` once it exits.

## When not to use teams

Most tasks are fine with a single agent + sub-agents via `Task`
(Chapter 15). Reach for teams only when the parallelism is real and
the overhead pays for itself. A good litmus: if you could hand each
teammate's task to a different human contractor without coordination
headaches, it's a team shape.
