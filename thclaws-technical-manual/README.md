# thClaws — Technical Manual

Engineering reference for thClaws contributors, advanced users, and
operators. Different audience than [`user-manual/`](../user-manual/) —
this manual assumes you are reading source, modifying behaviour, or
running infrastructure.

## Conventions

- **Topic-based files**, not numbered chapters. Reference docs are read
  by jumping to a topic, not front to back.
- **Code paths cited as `path/to/file.rs:LINE`** so you can grep or open
  them directly. Line numbers drift; the symbol name beside them is the
  durable part.
- **Wire-format and schema docs carry exact JSON** matching what ships,
  not synthetic examples.
- Runtime state lives under **`.thclaws/state/`** since workspace v2
  (2026-07-05). Docs predating that move have been corrected; if you
  find `.thclaws/kms/` or `.thclaws/sessions/` written anywhere, it is a
  bug in the doc.

## Start here

| File | What it covers |
|---|---|
| [`app-architecture.md`](app-architecture.md) | How the binary is put together: four surfaces over one engine, thread and channel topology, the wry/tao webview and Rust↔JS bridge, settings layering, the filesystem sandbox |
| [`agentic-loop.md`](agentic-loop.md) | The state machine that turns one user message into a streamed conversation with tool calls — the single most important file to read first |
| [`running-modes.md`](running-modes.md) | One engine, four surfaces (GUI, CLI REPL, print, `--serve`) and what differs between them |

## The turn

| File | What it covers |
|---|---|
| [`context-composer.md`](context-composer.md) | How the system prompt, tool definitions and history become the `StreamRequest` a provider receives |
| [`compaction.md`](compaction.md) | Keeping a growing conversation under the context window: five entry points, token budgeting, JSONL checkpoints |
| [`prompt-cache.md`](prompt-cache.md) | Cross-provider prompt caching: what `Usage` reports, which providers implement it, and what invalidates a cache |
| [`permissions.md`](permissions.md) | The three layered gates on tool execution — approval mode, the filesystem sandbox, and OS-level confinement |
| [`confinement.md`](confinement.md) | The OS-level layer under the shell tool: Seatbelt and Landlock, write roots, denied secrets, the multiuser read mask, and why the audit record reports mode *and* enforced |
| [`pii-masking.md`](pii-masking.md) | Opt-in Thai PII detection and pseudonymization before the wire: the plaintext-history invariant, why precision beats recall in a language without word spaces, and streaming restore |
| [`plan-mode.md`](plan-mode.md) | Read-only exploration then sequentially-gated execution: `EnterPlanMode`, step boundaries, the approval surface |
| [`sessions.md`](sessions.md) | Append-only JSONL conversation persistence, replay, rename, and the session store |
| [`todo.md`](todo.md) | The model's own scratchpad for multi-step work: full-state replacement, the reminder injection |

## Delegation and long-running work

| File | What it covers |
|---|---|
| [`subagent.md`](subagent.md) | Model-driven in-process delegation via the `Task` tool: fresh Agent, own registry, depth limits |
| [`side-channel.md`](side-channel.md) | User-driven concurrent agents (`/agent <name>`), parallel to the main turn with their own cancel token |
| [`agent-team.md`](agent-team.md) | Multi-process teammates coordinated through filesystem mailboxes, task queue, tmux panes, git worktrees |
| [`loop-and-goal.md`](loop-and-goal.md) | `/loop` fixed-interval iteration and `/goal` audit-driven completion — the overnight builder |
| [`schedule.md`](schedule.md) | Recurring jobs: in-process scheduler, native daemon (launchd / systemd-user), cron and workspace-watch triggers |
| [`workflows.md`](workflows.md) | Code-driven fan-out: the model authors a JS script, you review it, Boa executes it deterministically against real subagents |

## Tools and extension points

| File | What it covers |
|---|---|
| [`built-in-tools.md`](built-in-tools.md) | Every non-document built-in: filesystem, shell, search, web, media, and the registry contract a new tool must satisfy |
| [`document-tools.md`](document-tools.md) | The twelve Word / Excel / PowerPoint / PDF tools and their shared Create / Edit / Read surface |
| [`hooks.md`](hooks.md) | Shell commands fired on lifecycle events (`pre_tool_use`, `session_start`, …) and what each can veto |
| [`skills.md`](skills.md) | `SKILL.md` prompt+script bundles: discovery, the gate mechanism, install and trust model |
| [`plugins.md`](plugins.md) | Bundles of skills + commands + agents + MCP servers installed as one unit from git or a zip |
| [`commands.md`](commands.md) | Markdown slash-command templates that inject as a user message |
| [`mcp.md`](mcp.md) | The Model Context Protocol client: stdio and HTTP-Streamable transports, OAuth 2.1 + PKCE, tool namespacing |
| [`marketplace.md`](marketplace.md) | The curated skill / MCP / plugin registry: three-layer cache, schema, trust model, deployment |
| [`browser.md`](browser.md) | The engine-managed Chromium: Playwright MCP injection plus direct CDP for live view and human takeover |

## Knowledge and memory

| File | What it covers |
|---|---|
| [`kms.md`](kms.md) | Knowledge bases — pages, index, log, schema, provenance, backlinks, graph, verify, lint, import/export |
| [`memory.md`](memory.md) | The long-lived markdown memory store the agent reads and maintains, auto-loaded every turn |
| [`dream.md`](dream.md) | `/dream` — the built-in side-channel agent that consolidates a KMS by mining recent sessions |
| [`research.md`](research.md) | `/research` — background web search → iterative LLM synthesis → KMS write |

## Providers

| File | What it covers |
|---|---|
| [`providers.md`](providers.md) | The `Provider` trait, the `ProviderKind` enum, model-prefix routing, and the assembler that stitches wire events into content blocks |
| [`model-catalogue.md`](model-catalogue.md) | Per-model metadata — context window, max output, pricing, provenance — and how the catalogue is refreshed and shipped |
| [`provider-anthropic.md`](provider-anthropic.md) | The Anthropic Messages SSE format; three `ProviderKind` variants over one impl |
| [`provider-openai.md`](provider-openai.md) | The OpenAI Chat Completions SSE workhorse — most `ProviderKind` variants resolve here |
| [`provider-responses.md`](provider-responses.md) | OpenAI's newer `/v1/responses` API, separate from Chat Completions |
| [`provider-gemini.md`](provider-gemini.md) | Google's `generativelanguage` SSE format |
| [`provider-ollama.md`](provider-ollama.md) | The two local NDJSON variants (native and Anthropic-compatible) |
| [`provider-agentsdk.md`](provider-agentsdk.md) | The only non-HTTP provider: the `claude` CLI wrapped as a subprocess |
| [`provider-gateway.md`](provider-gateway.md) | The Enterprise org-gateway overlay — provider substitution inside `build_provider`, not a `Provider` impl |
| [`provider-thclaws-gateway.md`](provider-thclaws-gateway.md) | The thclaws.cloud metered-gateway overlay, same shape, different target |

## Serve mode and the HTTP surface

| File | What it covers |
|---|---|
| [`serve-mode.md`](serve-mode.md) | `--serve`: the Axum server, the WebSocket IPC bridge, the trust model, reconnect |
| [`openai-api.md`](openai-api.md) | `GET /v1/models` and `POST /v1/chat/completions` — sync, SSE, the tool-use extension, and async `x_callback` delivery |
| [`anthropic-api.md`](anthropic-api.md) | `POST /v1/messages` in Anthropic Messages shape, for the `anthropic` SDKs and `ANTHROPIC_BASE_URL` clients |
| [`agent-endpoint.md`](agent-endpoint.md) | `POST /agent/run` — the agent-shaped endpoint with per-request skill / MCP / plugin scoping |
| [`agent-info-endpoint.md`](agent-info-endpoint.md) | `GET /v1/agent/info` — the read-only capability snapshot an orchestrator reads before dispatching |
| [`job-artifacts.md`](job-artifacts.md) | Session-scoped, hash-frozen file transfer in and out for orchestrators |
| [`multi-tenant-serve.md`](multi-tenant-serve.md) | One pod, N users: per-user session, workspace and identity, and the forced auto-approve consequence |
| [`gui-shells.md`](gui-shells.md) | How a catalog agent ships its own HTML UI and how that UI talks to the engine |

## Chat bridges

| File | What it covers |
|---|---|
| [`line-bridge.md`](line-bridge.md) | LINE OA ↔ desktop relay: pairing, the relay server, approval routing |
| [`telegram-bridge.md`](telegram-bridge.md) | Telegram Bot API adapter — relay-free long polling, inline-keyboard approvals |
| [`messenger-bridge.md`](messenger-bridge.md) | Facebook Page Messenger bridge over the same relay as LINE |
| [`thclaws-remote.md`](thclaws-remote.md) | The outbound tunnel that makes a local agent reachable from the cloud with no inbound port — code-named `phone_home` |

## Cloud, identity and packaging

| File | What it covers |
|---|---|
| [`thclaws-cloud-client.md`](thclaws-cloud-client.md) | The engine side of thclaws.cloud: login, publish, get, agent identity, and what publishing strips |
| [`sso.md`](sso.md) | The OIDC Authorization-Code + PKCE flow, loopback redirect, keychain storage, silent refresh |
| [`enterprise-policy.md`](enterprise-policy.md) | Code map for `policy/` and `audit/`: the open-core rule, every site `policy::active()` is consulted, and the checklist for adding a block. ENTERPRISE.md is the behaviour reference |
| [`docker.md`](docker.md) | Container packaging for `thclaws --serve` |

## Media

| File | What it covers |
|---|---|
| [`media-generation.md`](media-generation.md) | The provider seam behind the image / video / speech tools: three traits, the registry, the async video job store, content-addressed output |
| [`filmscript.md`](filmscript.md) | `.film` screenplay → shot list → generated video, end to end |

