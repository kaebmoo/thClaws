# thClaws Multi-Machine Control Plane Plan

## Summary

thClaws already has enough API surface to act as a worker runtime on each
machine. The control plane should be built as a separate service above those
workers. It should register machines, decide where work should run, call
existing thClaws APIs, aggregate live output, and keep durable job/session
state.

Do not make thClaws itself the fleet controller yet. Keep thClaws focused on
running an agent inside one machine/workspace, and let the new service own
multi-machine orchestration.

## Naming

Recommendation: keep **NOVA** as the assistant/persona the user talks to, and
give the infrastructure layer a separate name. This prevents confusion between
"the AI assistant made a decision" and "the router/control plane moved a job".

Good candidates:

| Name | Fit |
|---|---|
| **Atlas** | Good default. Implies map of machines/workspaces and routing. |
| **Conductor** | Clear orchestration meaning, but slightly generic. |
| **Nexus** | Good for central hub, less operationally specific. |
| **Mission Control** | Very clear for a dashboard, a bit long as a service name. |
| **Dispatcher** | Boring and accurate. Good internal service name. |
| **Relay** | Good for message transport, too narrow for policy/routing. |

Recommended split:

```txt
NOVA          = user-facing assistant/persona
Atlas         = control plane service and dashboard
Hermes        = local bridge/gateway pattern, if kept
thClaws worker = runtime on each machine/workspace
```

## Target Architecture

```txt
Web / Telegram / API client
        |
        v
NOVA assistant layer
        |
        v
Atlas control plane
        |
        +-- thClaws worker: personal-machine
        |      GET  /healthz
        |      GET  /v1/agent/info
        |      POST /agent/run
        |
        +-- thClaws worker: company-a-machine
        |
        +-- thClaws worker: company-b-machine
```

Atlas should be responsible for:

- worker registry
- capability polling
- routing policy
- job queue and job state
- session mapping
- stream aggregation
- auth, audit, and operator UI

thClaws workers should remain responsible for:

- local workspace execution
- tools, skills, MCP, KMS, memory, and sessions inside that workspace
- token/provider configuration local to that machine
- filesystem sandbox and local process boundaries

## Existing thClaws APIs To Use

| Endpoint | Use |
|---|---|
| `GET /healthz` | Lightweight liveness check. |
| `GET /v1/agent/info` | Capability snapshot: version, workspace, skills, MCP servers, model capabilities, feature flags. |
| `POST /agent/run` | Main worker invocation API. Supports `prompt`, `workspace_dir`, `model`, `session_id`, `stream`, `max_tokens`, `x_callback`. |
| `GET /v1/models` | OpenAI-compatible model discovery. Useful for external clients and UI dropdowns. |
| `POST /v1/chat/completions` | OpenAI-compatible chat surface for generic tools. Not ideal for orchestrator routing because it has no workspace/capability envelope. |
| `POST /v1/deploy*` | Optional later use for pushing `.thclaws/` bundle/config to workers. |
| `POST /v1/restart` | Optional operator action after deploy/config changes. |

## Current Capability Matrix

### Works Now Without Changing thClaws

These can be implemented entirely in Atlas:

| Capability | How |
|---|---|
| Worker health | Poll `GET /healthz`. |
| Capability discovery | Poll `GET /v1/agent/info` and cache by worker id. |
| Manual routing | User or policy selects `worker_id` + `workspace_dir`; Atlas calls `/agent/run`. |
| Automatic routing MVP | Match prompt/workspace/company tags against worker capability snapshot and registry metadata. |
| Live output | Proxy SSE from `/agent/run` to web UI. |
| Async jobs | Use `/agent/run` with `x_callback`; Atlas receives terminal callback. |
| Multi-turn sessions | Store returned `session_id` per `(worker_id, workspace_id, conversation_id)`. |
| OpenAI-compatible access | Expose or proxy `/v1/models` and `/v1/chat/completions` where useful. |
| Audit at job level | Atlas logs request metadata, selected route, user, timestamps, final output summary. |
| Worker config deployment | Use `/v1/deploy*` for selected config bundles where the current deploy contract fits. |
| Worker restart | Call `/v1/restart` after deployment when appropriate. |
| Telegram "send to machine X" | Parse command in Atlas/NOVA, resolve route, call `/agent/run`. |

### Possible With Workarounds

These are usable, but the UX or guarantees are weaker until thClaws grows
native support:

| Capability | Workaround | Limitation |
|---|---|---|
| Cancel running stream job | Close client stream, apply Atlas timeout, or kill/restart worker as last resort. | No native `job_id -> cancel` API, especially weak for detached `x_callback` runs. |
| Job status | Atlas owns state: queued/running/done/failed based on its own dispatch and stream lifecycle. | Worker cannot list currently running jobs. |
| Stream reconnect | Atlas buffers SSE events and lets browser reconnect to Atlas. | If Atlas lost the worker stream, thClaws cannot resume that stream mid-run. |
| Approval gate | Pre-approve at Atlas policy layer before dispatch. Use safer worker permissions for trusted jobs. | No per-tool remote approval prompt from `/agent/run`. |
| Tool audit | Capture `/agent/run` SSE `tool_use_start` and `tool_use_result`. | Good for observed stream, but not a first-class central audit protocol with approvals. |
| Routing confidence | Use rules/tags first, LLM classifier second. | Capability snapshot is not a full semantic inventory of every project. |
| Team workflows | Open the target machine's thClaws UI/session directly, or ask the worker to proceed sequentially. | `/agent/run` does not register thClaws Team tools. |
| Config sync | Push bundles with `/v1/deploy*`, then restart. | Not a complete bidirectional config management system. |

### Not Native Without Changing thClaws

These need new thClaws API or runtime work for a clean production experience:

| Capability | Missing native surface |
|---|---|
| Cancel by job id | Worker-side job registry plus `POST /agent/jobs/{id}/cancel` or equivalent. |
| List running jobs | Worker-side `GET /agent/jobs` or status endpoint. |
| Per-tool remote approval for `/agent/run` | Approval request event, pending approval id, and approve/deny API. |
| Native stream resume | Durable event cursor per job and reconnect support. |
| Native structured Team API over HTTP | Remote create/list/spawn/message/merge team operations. |
| First-class central audit | Stable event schema for tool calls, approvals, denials, policy decisions, and file mutations. |
| Worker-side route policy | Today routing belongs outside thClaws. Native support would require declaring workspace/capability metadata in thClaws. |
| Multi-worker coordination inside thClaws | thClaws Agent Teams are per-project/process. Fleet coordination belongs in Atlas unless thClaws is extended. |

## Control Plane Data Model

Minimum tables/collections:

| Entity | Key fields |
|---|---|
| `workers` | `id`, `name`, `base_url`, `auth_ref`, `machine_role`, `status`, `last_seen_at`, `tags` |
| `workspaces` | `id`, `worker_id`, `workspace_dir`, `workspace_key`, `company`, `visibility`, `tags` |
| `capability_snapshots` | `worker_id`, `fetched_at`, raw `/v1/agent/info` JSON |
| `conversations` | `id`, `owner_user_id`, `workspace_id`, `default_model`, `title` |
| `session_bindings` | `conversation_id`, `worker_id`, `workspace_id`, `thclaws_session_id` |
| `jobs` | `id`, `conversation_id`, `worker_id`, `workspace_id`, `state`, `prompt_hash`, `route_reason`, `started_at`, `ended_at` |
| `job_events` | `job_id`, `seq`, `type`, `payload`, `created_at` |
| `route_rules` | `priority`, `match`, `target_worker_id`, `target_workspace_id`, `enabled` |
| `audit_logs` | `actor`, `action`, `target`, `metadata`, `created_at` |

Keep worker tokens in a secrets backend. Do not store raw worker tokens in
normal database rows.

## Atlas API Sketch

Internal API:

```http
GET  /workers
POST /workers
GET  /workers/{id}
POST /workers/{id}/poll

GET  /workspaces
POST /workspaces

POST /routes/resolve
POST /jobs
GET  /jobs/{id}
GET  /jobs/{id}/events
POST /jobs/{id}/cancel        # Atlas-level first; native worker cancel later

GET  /conversations/{id}
POST /conversations/{id}/messages
```

For browser streaming, prefer Server-Sent Events from Atlas:

```http
GET /jobs/{id}/events
```

Atlas can translate worker SSE events into stable UI events and persist them
with monotonically increasing `seq` values.

## Routing Strategy

Start deterministic, then add model-assisted classification only where useful.

1. Explicit target wins: user says "use company A machine" or selects a
   workspace in UI.
2. Workspace binding wins: existing conversation already has a worker and
   `thclaws_session_id`.
3. Route rules: company/project/tag maps to worker/workspace.
4. Capability filter: worker must be online and support `agent_run`.
5. Optional classifier: ask a small model to choose among eligible routes and
   return a reason.
6. Human fallback: if confidence is low, ask the user which workspace to use.

Route result should always record `route_reason` for audit and debugging.

## Implementation Phases

### Phase 0 - Worker Registry And Manual Dispatch

Goal: send work to a selected worker and see the live stream.

Deliverables:

- Worker registry with base URL and token reference.
- Health poll using `/healthz`.
- Capability poll using `/v1/agent/info`.
- Manual job creation with `worker_id`, `workspace_dir`, `prompt`.
- SSE proxy from `/agent/run` to Atlas UI.
- Persist `session_id` returned by thClaws.
- Minimal job states: `queued`, `running`, `succeeded`, `failed`.

Exit criteria:

- One Atlas screen can send a prompt to two different machines and show which
  machine handled each job.

### Phase 1 - Workspace-Aware Routing

Goal: user talks to NOVA/Atlas without manually choosing the machine every time.

Deliverables:

- Workspace registry and tags.
- Route rules by company/project/task type.
- Conversation to session binding.
- Route explanation displayed in UI.
- Telegram command support: explicit machine/workspace selection.

Exit criteria:

- "Run this in company A" consistently routes to the correct worker/workspace.
- Follow-up messages continue the same thClaws session.

### Phase 2 - Durable Job Events And Async Mode

Goal: long jobs survive browser disconnects.

Deliverables:

- Persist worker SSE events into `job_events`.
- Browser reconnect to Atlas event stream by `seq`.
- `x_callback` receiver for detached worker jobs.
- Timeouts and retry policy at Atlas level.
- Best-effort cancel: mark cancelled in Atlas, close stream if attached, prevent
  callbacks from reviving cancelled jobs.

Exit criteria:

- Closing the browser does not lose the job transcript.
- Async jobs complete via callback and update the dashboard.

### Phase 3 - Operations And Security

Goal: make it safe enough for daily use across personal/company machines.

Deliverables:

- Per-user auth and RBAC.
- Worker token rotation workflow.
- Audit log for dispatch, route decisions, config deploy, restart.
- Optional `/v1/deploy*` integration for managed `.thclaws/` bundles.
- Worker status page: online/offline, last capability snapshot, current jobs
  known by Atlas.

Exit criteria:

- A user can operate multiple machines without exposing raw worker tokens or
  guessing which worker is live.

### Phase 4 - Optional thClaws Enhancements

Goal: remove workarounds once the MVP proves the control plane shape.

Candidate thClaws changes, in priority order:

1. Native job id, job status, and cancel API for `/agent/run`.
2. Remote approval protocol for `/agent/run`.
3. Durable stream cursor and resume support.
4. Structured tool/audit event schema.
5. HTTP surface for Agent Team operations, if remote teams become important.

Do not start here. Build Atlas around existing APIs first; add thClaws changes
only after the operational pain is proven.

## Practical MVP Defaults

- Use Tailscale, Cloudflare Access, or SSH tunnel between Atlas and workers.
- Require `THCLAWS_API_TOKEN` on every worker.
- Set `THCLAWS_AGENT_WORKSPACE_ROOT` on workers that accept explicit
  `workspace_dir`.
- Prefer `/agent/run` over `/v1/chat/completions` for Atlas dispatch.
- Keep OpenAI-compatible `/v1/chat/completions` for third-party tools.
- Use deterministic routing rules before adding an LLM router.
- Keep NOVA as the conversational face; keep Atlas as the boring control plane.

## Open Questions

- Should Atlas own worker credentials per user, per machine, or per workspace?
- Should workers be pull-based in some environments, instead of Atlas calling
  them directly?
- Should Telegram commands support explicit prefixes such as
  `/to company-a ...`, or should routing be inferred from text?
- What is the minimum approval posture acceptable for each worker type?
- Which machine is allowed to touch cross-company memory, if any?
- Should control plane state be local-first SQLite initially, or server DB from
  day one?
