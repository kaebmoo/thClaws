# Anthropic Messages–compatible API

`thclaws --serve` exposes `POST /v1/messages` in the Anthropic Messages
wire shape, alongside the OpenAI-shaped
[`/v1/chat/completions`](openai-api.md). Same listener, same agent, same
toolset — only the request and response encoding differ.

This is what you point at when the client already speaks Anthropic:

- the `anthropic` Python / TypeScript SDKs
- anything driven by `ANTHROPIC_BASE_URL` + `ANTHROPIC_API_KEY`
- LiteLLM's `anthropic/` provider prefix

Source: [`crates/core/src/api_v1/messages.rs`](../crates/core/src/api_v1/messages.rs).
The agent itself is built by `chat::build_agent_with`, shared with the
OpenAI endpoint so the two surfaces cannot drift apart in which tools
they expose.

> **This does not make thClaws a Claude proxy.** The request names a
> model and thClaws runs *its own agent loop* with that model — reading
> files, running commands, calling tools on the machine hosting
> `--serve`. The response is the agent's final answer, not a raw model
> completion. That is the same contract as the OpenAI endpoint; see
> [`openai-api.md`](openai-api.md) §external-client surface.

## Quick start

```sh
# 1. Start the server with an API token
THCLAWS_API_TOKEN=secret123 thclaws --serve --port 7878

# 2. From any Anthropic-compatible client
curl http://localhost:7878/v1/messages \
  -H "x-api-key: secret123" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-sonnet-5",
    "max_tokens": 1024,
    "messages": [{"role": "user", "content": "what does src/main.rs do?"}]
  }'
```

The server needs whatever provider credentials the chosen model
requires (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, …) in its own
environment — exactly as for the OpenAI endpoint.

## Authentication

`/v1/messages` accepts the `THCLAWS_API_TOKEN` in **either** header:

| Header | Sent by |
|---|---|
| `x-api-key: <token>` | the Anthropic SDKs, `ANTHROPIC_API_KEY` |
| `Authorization: Bearer <token>` | everything else |

Both are compared in constant time, and both are checked on every
request — there is no short-circuit on the first one present, so timing
does not reveal which header was used.

The three token modes are the same as the rest of `/v1/*`
([`openai-api.md` §Authentication](openai-api.md#authentication)):
`THCLAWS_API_TOKEN` unset ⇒ the whole API 404s; `disable-auth` ⇒ no
header required (loopback binds only); any other value ⇒ must match.

`anthropic-version` is accepted and ignored. thClaws implements one
version of the shape and does not vary behaviour by header.

## Request

```json
{
  "model": "claude-sonnet-5",
  "max_tokens": 1024,
  "system": "You are reviewing a Rust codebase.",
  "messages": [
    {"role": "user", "content": "summarize the agent loop"},
    {"role": "assistant", "content": "It is a streaming state machine…"},
    {"role": "user", "content": "where does it stop?"}
  ],
  "stream": false
}
```

| Field | Handling |
|---|---|
| `model` | Passed through to the agent's config. Empty string ⇒ the server's configured default model |
| `max_tokens` | **Required**, as in the real API. A missing field is a 400; `0` is a 400 |
| `messages` | The **last `user` message is the prompt**; everything before it becomes conversation history. Roles other than `user`/`assistant` are dropped |
| `system` | Top-level, string or `[{"type":"text",…}]` blocks. **Appended** to thClaws' own system prompt, never replacing it — the default carries the tool-aware scaffolding the agent needs |
| `stream` | `false` ⇒ one JSON body; `true` ⇒ SSE (below) |
| `temperature`, `stop_sequences`, `top_p`, `top_k`, `metadata` | Accepted, currently not applied |
| `tools`, `tool_choice` | Accepted and **ignored** — thClaws' tools are internal to its agent loop and are not caller-injectable on this surface. Use [`POST /agent/run`](agent-endpoint.md) if you need per-request tool scoping |
| `x_thclaws_tool_events` | thClaws extension, default `false`. See §Tool visibility |

**Content blocks.** `content` may be a string or an array. Text blocks
are concatenated with `\n`; `image`, `tool_result` and `document` blocks
are dropped — this endpoint is text-in, text-out.

## Response (non-streaming)

```json
{
  "id": "msg_thc_17f3c2a91b4e",
  "type": "message",
  "role": "assistant",
  "model": "claude-sonnet-5",
  "content": [{"type": "text", "text": "The loop stops when…"}],
  "stop_reason": "end_turn",
  "stop_sequence": null,
  "usage": {
    "input_tokens": 5122,
    "output_tokens": 318,
    "cache_read_input_tokens": 4096
  }
}
```

`content` always carries exactly one text block. The agent may have made
many tool calls to produce it; those are not represented as
`tool_use` / `tool_result` blocks, because they were already executed
server-side rather than being handed back for the caller to run.

**`stop_reason`** is normalised to Anthropic's four values, because the
model underneath may not be an Anthropic one:

| Upstream reported | Reported here |
|---|---|
| `end_turn`, `stop`, absent | `end_turn` |
| `max_tokens`, `length` | `max_tokens` |
| `stop_sequence` | `stop_sequence` |
| `tool_use`, `tool_calls` | `tool_use` |
| anything else | `end_turn` |

The catch-all matters: an SDK deserialises `stop_reason` into an enum,
so leaking an unmapped value through would be a client-side crash.

**`usage`** carries `input_tokens` / `output_tokens` always, plus
`cache_read_input_tokens` / `cache_creation_input_tokens` when the
provider reported them. Absent counts are omitted rather than sent as
`null`, matching the SDK's optional-int typing.

## Response (streaming)

`"stream": true` returns `text/event-stream` in Anthropic's documented
event sequence:

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_thc_…","type":"message","role":"assistant","model":"claude-sonnet-5","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":0,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"The loop "}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"stops when…"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":318,"input_tokens":5122}}

event: message_stop
data: {"type":"message_stop"}
```

Two things to know:

- **No `[DONE]` sentinel.** That is an OpenAI convention; Anthropic
  streams end at `message_stop`.
- **`message_start` reports `input_tokens: 0`.** thClaws does not know
  the input count until the provider reports it at the end of the turn,
  so the real counts land in `message_delta`. `input_tokens` there is an
  additive field (the real API only sends `output_tokens`); SDKs ignore
  it, and a caller metering thClaws has nowhere else to read it on a
  streamed request.

A `:keepalive` comment goes out every 15 s so proxies don't drop the
connection while the agent is working. Spec-compliant SSE parsers
ignore comments.

**Mid-stream failure.** The response headers are already flushed by the
time the agent can fail, so an error cannot become a 5xx. It is emitted
as a `content_block_delta` of `\n\n[thclaws error] …` and the stream is
then closed properly through `content_block_stop` → `message_delta` →
`message_stop`. An SDK that never saw the closing frames would raise a
parse error instead of showing the operator what actually broke.

## Tool visibility

By default the stream is **strictly spec-shaped**: the only event names
emitted are the six above. The OpenAI endpoint can attach tool activity
to a chunk as an extra JSON field (`x_thclaws_tool_use`) because a
strict client just ignores an unknown key — but Anthropic SDKs dispatch
on the `event:` name, and an unknown event type is not something a
client can ignore the same way.

Set `"x_thclaws_tool_events": true` in the request to opt in. thClaws
then interleaves:

```
event: thclaws_tool_use
data: {"type":"thclaws_tool_use","id":"toolu_…","name":"Bash","status":"started","input":{"command":"cargo test"}}

event: thclaws_tool_use
data: {"type":"thclaws_tool_use","id":"toolu_…","name":"Bash","status":"completed","output":{"preview":"running 2431 tests…","truncated":true,"total_chars":9814}}
```

`status` is `started` / `completed` / `error` / `denied`. Outputs are
truncated to a 400-byte preview on a UTF-8 char boundary, with
`truncated` and `total_chars` so the caller knows what it is missing.

Only enable this for a client you control.

## Errors

The Anthropic envelope, which is what `anthropic.APIStatusError` parses:

```json
{"type": "error", "error": {"type": "authentication_error", "message": "Invalid API key. …"}}
```

| Status | `error.type` | When |
|---|---|---|
| 404 | — (plain text `api disabled`) | `THCLAWS_API_TOKEN` unset |
| 401 | `authentication_error` | Token missing or wrong in both headers |
| 400 | `invalid_request_error` | `max_tokens` absent or `0`, empty `messages`, no `user` message |
| 500 | `api_error` | Provider/agent failure before the stream opened |

## Not implemented

| | Why / what to use instead |
|---|---|
| `GET /v1/models` in Anthropic shape | That path already serves the OpenAI model list. Listing models is not required to send a message; use the OpenAI shape or the catalogue |
| `POST /v1/messages/count_tokens` | Not implemented |
| `POST /v1/messages/batches` | Not implemented. For fire-and-forget work use [`x_callback`](openai-api.md#async-mode-x_callback-extension) on the OpenAI endpoint |
| Caller-supplied `tools` | thClaws' tools are internal; see [`agent-endpoint.md`](agent-endpoint.md) for per-request tool scoping |
| Vision / document blocks | Text blocks only |
| Extended thinking blocks | The agent's thinking is not surfaced on this endpoint |

## See also

- [`openai-api.md`](openai-api.md) — the sibling `/v1/chat/completions` surface, plus the shared auth model, working-directory semantics and `x_callback` async mode.
- [`agent-endpoint.md`](agent-endpoint.md) — `POST /agent/run`, the agent-shaped endpoint for orchestrators that need skill / MCP / plugin injection per request.
- [`provider-gateway.md`](provider-gateway.md) — the *other* direction: routing thClaws' own outbound model calls through an org gateway. Note the org-gateway substitution builds an OpenAI Chat Completions client, so an Anthropic-Messages-only gateway is not supported there.
