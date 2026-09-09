//! `POST /v1/messages` — Anthropic Messages–compatible endpoint.
//!
//! The sibling of [`super::chat`]: same agent, same toolset, same
//! `--serve` listener, different wire shape. Anything that speaks the
//! Anthropic Messages API — the `anthropic` Python/TS SDKs, Claude Code
//! pointed at a custom `ANTHROPIC_BASE_URL`, LiteLLM's anthropic
//! provider — can drive thClaws without a translation layer.
//!
//! Both endpoints build the agent through
//! [`super::chat::build_agent_with`] so their tool surface cannot drift.
//!
//! Two deliberate differences from the OpenAI endpoint:
//!
//! - **Auth accepts `x-api-key` as well as `Authorization: Bearer`.**
//!   The Anthropic SDKs send `x-api-key`; refusing it would mean every
//!   caller needs a custom header hook, which defeats the point.
//! - **The stream is strictly spec-shaped by default.** The official
//!   SDKs match on `event:` names, so an unknown event type is not a
//!   field a client can ignore the way it ignores an unknown JSON key.
//!   Tool-call visibility is therefore opt-in via
//!   `x_thclaws_tool_events` rather than always-on.
//!
//! Not implemented: `tools` / `tool_choice` in the request (thClaws'
//! tools are internal — the field is accepted and ignored), and
//! `/v1/messages/count_tokens`. `GET /v1/models` stays OpenAI-shaped;
//! it is a different endpoint's contract and listing models is not
//! required to send a message.

use axum::extract::FromRequestParts;
use axum::http::{request::Parts, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

use crate::agent::{collect_agent_turn, AgentEvent, AgentTurnOutcome};
use crate::providers::Usage;
use crate::types::{ContentBlock, Message, Role};

// ── auth ──────────────────────────────────────────────────────────────

/// Same token policy as [`super::AuthOk`], but the token may arrive in
/// either `x-api-key` (what the Anthropic SDKs send) or
/// `Authorization: Bearer` (what everything else sends). Rejections use
/// the Anthropic error envelope so SDK error handling works.
pub struct AnthropicAuthOk;

impl<S: Send + Sync> FromRequestParts<S> for AnthropicAuthOk {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let expected = match super::auth_token_for_messages() {
            None => {
                // API disabled entirely — same 404 as the rest of /v1.
                return Err((StatusCode::NOT_FOUND, "api disabled").into_response());
            }
            Some(None) => return Ok(AnthropicAuthOk), // bypass
            Some(Some(t)) => t,
        };

        let from_x_api_key = parts
            .headers
            .get("x-api-key")
            .and_then(|h| h.to_str().ok())
            .unwrap_or_default();
        let from_bearer = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or_default()
            .strip_prefix("Bearer ")
            .unwrap_or_default();

        // Check both unconditionally — no short-circuit on the first
        // header, so timing doesn't reveal which one was populated.
        let ok_key = super::constant_time_eq(from_x_api_key.as_bytes(), expected.as_bytes());
        let ok_bearer = super::constant_time_eq(from_bearer.as_bytes(), expected.as_bytes());
        if ok_key || ok_bearer {
            Ok(AnthropicAuthOk)
        } else {
            Err((
                StatusCode::UNAUTHORIZED,
                Json(AnthropicError::authentication()),
            )
                .into_response())
        }
    }
}

// ── error envelope ────────────────────────────────────────────────────

/// `{"type":"error","error":{"type":"...","message":"..."}}` — the shape
/// `anthropic.APIStatusError` parses.
#[derive(Serialize)]
pub struct AnthropicError {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub error: AnthropicErrorBody,
}

#[derive(Serialize)]
pub struct AnthropicErrorBody {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub message: String,
}

impl AnthropicError {
    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: "error",
            error: AnthropicErrorBody {
                kind,
                message: message.into(),
            },
        }
    }
    pub fn authentication() -> Self {
        Self::new(
            "authentication_error",
            "Invalid API key. Set THCLAWS_API_TOKEN on the server, then send it as `x-api-key: <token>` or `Authorization: Bearer <token>`.",
        )
    }
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new("invalid_request_error", message)
    }
    pub fn api_error(message: impl Into<String>) -> Self {
        Self::new("api_error", message)
    }
}

// ── request shape ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct MessagesRequest {
    pub model: String,
    /// Required by the Anthropic API. Enforced here too: a client that
    /// omits it is misconfigured, and silently substituting a default
    /// would hide that until the bill arrived.
    pub max_tokens: u32,
    pub messages: Vec<InputMessage>,
    /// Top-level, unlike OpenAI where system is a message role.
    #[serde(default)]
    pub system: Option<SystemPrompt>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,
    /// thClaws extension: emit `event: thclaws_tool_use` frames as the
    /// agent runs tools. Off by default — see the module docs.
    #[serde(default)]
    pub x_thclaws_tool_events: bool,
    // `tools`, `tool_choice`, `metadata`, `top_p`, `top_k` are accepted
    // and ignored: thClaws' tools are internal to the agent loop.
}

#[derive(Deserialize)]
pub struct InputMessage {
    pub role: String,
    pub content: MessageContent,
}

/// Anthropic content is a string or an array of blocks.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<serde_json::Value>),
}

impl MessageContent {
    fn as_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => flatten_text_blocks(blocks),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum SystemPrompt {
    Text(String),
    Blocks(Vec<serde_json::Value>),
}

impl SystemPrompt {
    fn as_text(&self) -> String {
        match self {
            SystemPrompt::Text(s) => s.clone(),
            SystemPrompt::Blocks(blocks) => flatten_text_blocks(blocks),
        }
    }
}

/// Pull the `text` out of every `{"type":"text","text":…}` block and
/// join. Image / tool_result / document blocks are dropped — this
/// endpoint is text-in, text-out.
fn flatten_text_blocks(blocks: &[serde_json::Value]) -> String {
    blocks
        .iter()
        .filter_map(|b| {
            let obj = b.as_object()?;
            match obj.get("type").and_then(|t| t.as_str())? {
                "text" => obj.get("text").and_then(|t| t.as_str()).map(String::from),
                _ => None,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── response shape ────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub role: &'static str,
    pub model: String,
    pub content: Vec<TextBlock>,
    pub stop_reason: String,
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
}

#[derive(Serialize)]
pub struct TextBlock {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub text: String,
}

#[derive(Serialize, Default)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
}

// ── handler ───────────────────────────────────────────────────────────

pub async fn messages(
    _auth: AnthropicAuthOk,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, Response> {
    // Deserialize by hand rather than through `Json<MessagesRequest>`:
    // axum's own rejection is a plain-text 422, and an SDK parsing that
    // as an Anthropic error envelope reports something unhelpful. The
    // most likely way to land here is a client that omitted the
    // required `max_tokens`, which deserves a message saying so.
    let req: MessagesRequest = serde_json::from_value(body).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AnthropicError::invalid_request(format!(
                "could not parse request body: {e}"
            ))),
        )
            .into_response()
    })?;

    validate(&req).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AnthropicError::invalid_request(e)),
        )
            .into_response()
    })?;

    if req.stream {
        return messages_stream(req).await;
    }

    let model = req.model.clone();
    let (history, prompt, system) = split_messages(&req).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AnthropicError::invalid_request(e)),
        )
            .into_response()
    })?;

    let agent =
        super::chat::build_agent_with(&req.model, Some(req.max_tokens), system).map_err(|e| {
            let msg = format!("{e}");
            eprintln!("[api_v1] messages failure: {msg}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnthropicError::api_error(msg)),
            )
                .into_response()
        })?;
    agent.set_history(history);

    let outcome = collect_agent_turn(agent.run_turn(prompt))
        .await
        .map_err(|e| {
            let msg = format!("{e}");
            eprintln!("[api_v1] messages failure: {msg}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnthropicError::api_error(msg)),
            )
                .into_response()
        })?;

    Ok(Json(build_messages_response(model, outcome)).into_response())
}

/// SSE in Anthropic's event sequence:
/// `message_start` → `content_block_start` → `content_block_delta`* →
/// `content_block_stop` → `message_delta` → `message_stop`.
/// No `[DONE]` sentinel — that is an OpenAI convention.
async fn messages_stream(req: MessagesRequest) -> Result<Response, Response> {
    let model = req.model.clone();
    let tool_events = req.x_thclaws_tool_events;
    let (history, prompt, system) = split_messages(&req).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AnthropicError::invalid_request(e)),
        )
            .into_response()
    })?;

    let agent =
        super::chat::build_agent_with(&req.model, Some(req.max_tokens), system).map_err(|e| {
            let msg = format!("{e}");
            eprintln!("[api_v1] messages_stream setup failure: {msg}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnthropicError::api_error(msg)),
            )
                .into_response()
        })?;
    agent.set_history(history);

    let msg_id = message_id();
    let stream = async_stream::stream! {
        yield Ok::<_, Infallible>(sse("message_start", message_start_body(&msg_id, &model)));
        yield Ok(sse("content_block_start", content_block_start_body()));

        let mut turn = Box::pin(agent.run_turn(prompt));
        let mut final_usage: Option<Usage> = None;
        let mut final_stop: Option<String> = None;

        while let Some(ev) = turn.next().await {
            match ev {
                Ok(AgentEvent::Text(s)) => {
                    if !s.is_empty() {
                        yield Ok(sse("content_block_delta", text_delta_body(&s)));
                    }
                }
                Ok(AgentEvent::Done { stop_reason, usage }) => {
                    final_stop = stop_reason;
                    final_usage = Some(usage);
                }
                Ok(AgentEvent::ToolCallStart { id, name, input }) if tool_events => {
                    yield Ok(sse("thclaws_tool_use", serde_json::json!({
                        "type": "thclaws_tool_use",
                        "id": id, "name": name, "status": "started", "input": input,
                    })));
                }
                Ok(AgentEvent::ToolCallResult { id, name, output, .. }) if tool_events => {
                    let (status, text) = match &output {
                        Ok(s) => ("completed", s.clone()),
                        Err(e) => ("error", e.clone()),
                    };
                    yield Ok(sse("thclaws_tool_use", serde_json::json!({
                        "type": "thclaws_tool_use",
                        "id": id, "name": name, "status": status,
                        "output": truncate_preview(&text),
                    })));
                }
                Ok(AgentEvent::ToolCallDenied { id, name }) if tool_events => {
                    yield Ok(sse("thclaws_tool_use", serde_json::json!({
                        "type": "thclaws_tool_use",
                        "id": id, "name": name, "status": "denied",
                    })));
                }
                Ok(_) => {}
                Err(e) => {
                    // Headers are already flushed, so we can't 5xx. Emit
                    // the failure as text and close the frames cleanly —
                    // an SDK that never sees content_block_stop /
                    // message_stop raises a confusing parse error
                    // instead of showing the operator what broke.
                    let msg = format!("\n\n[thclaws error] {e}");
                    yield Ok(sse("content_block_delta", text_delta_body(&msg)));
                    final_stop = Some("error".into());
                    break;
                }
            }
        }

        yield Ok(sse("content_block_stop", serde_json::json!({
            "type": "content_block_stop", "index": 0,
        })));
        yield Ok(sse("message_delta", message_delta_body(
            &map_stop_reason(final_stop.as_deref()),
            final_usage.as_ref(),
        )));
        yield Ok(sse("message_stop", serde_json::json!({ "type": "message_stop" })));
    };

    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::new())
        .into_response())
}

// ── translation ───────────────────────────────────────────────────────

fn validate(req: &MessagesRequest) -> Result<(), String> {
    if req.max_tokens == 0 {
        return Err("max_tokens must be greater than 0".into());
    }
    if req.messages.is_empty() {
        return Err("messages: at least one message is required".into());
    }
    Ok(())
}

/// `(history before the last user message, that message's text, system)`.
///
/// Anthropic puts `system` at the top level, so unlike the OpenAI path
/// there is no system role to sift out of the array.
fn split_messages(req: &MessagesRequest) -> Result<(Vec<Message>, String, Option<String>), String> {
    let last_user_idx = req
        .messages
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, m)| (m.role == "user").then_some(i))
        .ok_or_else(|| "messages: no user message in request".to_string())?;

    let mut history = Vec::new();
    for (i, m) in req.messages.iter().enumerate() {
        if i == last_user_idx {
            continue;
        }
        let role = match m.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            // Anthropic only defines user/assistant; anything else is a
            // client bug. Drop it rather than fail the whole request.
            _ => continue,
        };
        history.push(Message {
            role,
            content: vec![ContentBlock::text(m.content.as_text())],
        });
    }

    let prompt = req.messages[last_user_idx].content.as_text();
    let system = req
        .system
        .as_ref()
        .map(|s| s.as_text())
        .filter(|s| !s.trim().is_empty());
    Ok((history, prompt, system))
}

fn build_messages_response(model: String, outcome: AgentTurnOutcome) -> MessagesResponse {
    let usage = outcome.usage.unwrap_or_default();
    MessagesResponse {
        id: message_id(),
        kind: "message",
        role: "assistant",
        model,
        content: vec![TextBlock {
            kind: "text",
            text: outcome.text,
        }],
        stop_reason: map_stop_reason(outcome.stop_reason.as_deref()),
        stop_sequence: None,
        usage: anthropic_usage(&usage),
    }
}

/// Normalize whatever the underlying provider reported down to
/// Anthropic's four canonical values. thClaws may be talking to OpenAI
/// or Gemini upstream, so `stop` / `length` / `tool_calls` all arrive
/// here and have to be translated back.
fn map_stop_reason(stop: Option<&str>) -> String {
    match stop {
        Some("end_turn") | Some("stop") | None => "end_turn".into(),
        Some("max_tokens") | Some("length") => "max_tokens".into(),
        Some("stop_sequence") => "stop_sequence".into(),
        Some("tool_use") | Some("tool_calls") => "tool_use".into(),
        // Unknown values (including our own "error") would break a
        // client matching on the enum — report the turn as ended.
        Some(_) => "end_turn".into(),
    }
}

fn anthropic_usage(u: &Usage) -> AnthropicUsage {
    AnthropicUsage {
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
        cache_creation_input_tokens: u.cache_creation_input_tokens,
        cache_read_input_tokens: u.cache_read_input_tokens,
    }
}

// ── SSE frame builders ────────────────────────────────────────────────

fn sse(event: &str, body: serde_json::Value) -> Event {
    Event::default().event(event).data(body.to_string())
}

fn message_start_body(id: &str, model: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "message_start",
        "message": {
            "id": id,
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": [],
            "stop_reason": null,
            "stop_sequence": null,
            // Input tokens aren't known until the provider reports them
            // at the end of the turn; the real API fills this in here.
            // Zero is the honest placeholder — the true counts land in
            // the message_delta frame below.
            "usage": { "input_tokens": 0, "output_tokens": 0 },
        },
    })
}

fn content_block_start_body() -> serde_json::Value {
    serde_json::json!({
        "type": "content_block_start",
        "index": 0,
        "content_block": { "type": "text", "text": "" },
    })
}

fn text_delta_body(text: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": { "type": "text_delta", "text": text },
    })
}

fn message_delta_body(stop_reason: &str, usage: Option<&Usage>) -> serde_json::Value {
    let mut body = serde_json::json!({
        "type": "message_delta",
        "delta": { "stop_reason": stop_reason, "stop_sequence": null },
        "usage": { "output_tokens": usage.map(|u| u.output_tokens).unwrap_or(0) },
    });
    // Input tokens are not part of Anthropic's message_delta usage, but
    // a caller metering thClaws needs them and has nowhere else to look
    // on a streamed request. Additive field, ignored by the SDKs.
    if let Some(u) = usage {
        body["usage"]["input_tokens"] = serde_json::json!(u.input_tokens);
    }
    body
}

const TOOL_OUTPUT_PREVIEW_LIMIT: usize = 400;

fn truncate_preview(out: &str) -> serde_json::Value {
    if out.len() <= TOOL_OUTPUT_PREVIEW_LIMIT {
        return serde_json::json!({
            "preview": out, "truncated": false, "total_chars": out.len(),
        });
    }
    let mut cut = TOOL_OUTPUT_PREVIEW_LIMIT;
    while !out.is_char_boundary(cut) {
        cut -= 1;
    }
    serde_json::json!({
        "preview": &out[..cut], "truncated": true, "total_chars": out.len(),
    })
}

fn message_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("msg_thc_{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(json: serde_json::Value) -> MessagesRequest {
        serde_json::from_value(json).expect("request parses")
    }

    #[test]
    fn string_content_and_top_level_system_are_read() {
        let r = req(serde_json::json!({
            "model": "claude-sonnet-5",
            "max_tokens": 64,
            "system": "be terse",
            "messages": [{ "role": "user", "content": "hello" }],
        }));
        let (history, prompt, system) = split_messages(&r).unwrap();
        assert!(history.is_empty());
        assert_eq!(prompt, "hello");
        assert_eq!(system.as_deref(), Some("be terse"));
    }

    #[test]
    fn block_content_flattens_text_and_drops_images() {
        let r = req(serde_json::json!({
            "model": "m", "max_tokens": 8,
            "system": [{ "type": "text", "text": "sys one" }],
            "messages": [{ "role": "user", "content": [
                { "type": "text", "text": "line one" },
                { "type": "image", "source": { "type": "base64", "data": "…" } },
                { "type": "text", "text": "line two" },
            ]}],
        }));
        let (_, prompt, system) = split_messages(&r).unwrap();
        assert_eq!(prompt, "line one\nline two");
        assert_eq!(system.as_deref(), Some("sys one"));
    }

    #[test]
    fn earlier_turns_become_history_and_last_user_is_the_prompt() {
        let r = req(serde_json::json!({
            "model": "m", "max_tokens": 8,
            "messages": [
                { "role": "user", "content": "first" },
                { "role": "assistant", "content": "reply" },
                { "role": "user", "content": "second" },
            ],
        }));
        let (history, prompt, system) = split_messages(&r).unwrap();
        assert_eq!(prompt, "second");
        assert_eq!(history.len(), 2);
        assert!(matches!(history[0].role, Role::User));
        assert!(matches!(history[1].role, Role::Assistant));
        assert!(system.is_none());
    }

    #[test]
    fn a_request_with_no_user_message_is_rejected() {
        let r = req(serde_json::json!({
            "model": "m", "max_tokens": 8,
            "messages": [{ "role": "assistant", "content": "orphan" }],
        }));
        assert!(split_messages(&r).is_err());
    }

    #[test]
    fn max_tokens_is_required_and_must_be_positive() {
        // Absent entirely: serde rejects it, matching the real API. The
        // handler deserializes by hand precisely so this turns into an
        // Anthropic-shaped 400 rather than axum's plain-text 422.
        let missing: Result<MessagesRequest, _> = serde_json::from_value(serde_json::json!({
            "model": "m",
            "messages": [{ "role": "user", "content": "hi" }],
        }));
        let err = match missing {
            Err(e) => e,
            Ok(_) => panic!("max_tokens is required"),
        };
        assert!(err.to_string().contains("max_tokens"), "{err}");

        let zero = req(serde_json::json!({
            "model": "m", "max_tokens": 0,
            "messages": [{ "role": "user", "content": "hi" }],
        }));
        assert!(validate(&zero).is_err());
    }

    #[test]
    fn empty_messages_is_rejected_before_the_agent_is_built() {
        let r = req(serde_json::json!({
            "model": "m", "max_tokens": 8, "messages": [],
        }));
        assert!(validate(&r).is_err());
    }

    #[test]
    fn unknown_fields_from_the_sdk_are_ignored() {
        // tools / tool_choice / metadata / top_p are real Anthropic
        // fields we don't act on — they must not fail the parse.
        let r = req(serde_json::json!({
            "model": "m", "max_tokens": 8,
            "messages": [{ "role": "user", "content": "hi" }],
            "tools": [{ "name": "get_weather", "input_schema": {} }],
            "tool_choice": { "type": "auto" },
            "metadata": { "user_id": "u1" },
            "top_p": 0.9, "top_k": 40,
        }));
        assert_eq!(r.max_tokens, 8);
    }

    #[test]
    fn provider_stop_reasons_map_onto_anthropics_four() {
        assert_eq!(map_stop_reason(Some("end_turn")), "end_turn");
        assert_eq!(map_stop_reason(Some("stop")), "end_turn");
        assert_eq!(map_stop_reason(None), "end_turn");
        assert_eq!(map_stop_reason(Some("length")), "max_tokens");
        assert_eq!(map_stop_reason(Some("max_tokens")), "max_tokens");
        assert_eq!(map_stop_reason(Some("tool_calls")), "tool_use");
        assert_eq!(map_stop_reason(Some("stop_sequence")), "stop_sequence");
        // Anything unrecognised must still be a legal enum value.
        assert_eq!(map_stop_reason(Some("error")), "end_turn");
        assert_eq!(map_stop_reason(Some("banana")), "end_turn");
    }

    #[test]
    fn non_stream_response_has_the_anthropic_shape() {
        let outcome = AgentTurnOutcome {
            text: "hi there".into(),
            tool_calls: vec![],
            tool_denials: vec![],
            stop_reason: Some("end_turn".into()),
            usage: Some(Usage {
                input_tokens: 11,
                output_tokens: 3,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: Some(7),
                reasoning_output_tokens: None,
            }),
            iterations: 1,
        };
        let v = serde_json::to_value(build_messages_response("claude-sonnet-5".into(), outcome))
            .unwrap();
        assert_eq!(v["type"], "message");
        assert_eq!(v["role"], "assistant");
        assert_eq!(v["content"][0]["type"], "text");
        assert_eq!(v["content"][0]["text"], "hi there");
        assert_eq!(v["stop_reason"], "end_turn");
        assert!(v["stop_sequence"].is_null());
        assert_eq!(v["usage"]["input_tokens"], 11);
        assert_eq!(v["usage"]["output_tokens"], 3);
        assert_eq!(v["usage"]["cache_read_input_tokens"], 7);
        // Absent cache-write must be omitted, not null — the SDK's
        // usage model types it as an optional int.
        assert!(v["usage"].get("cache_creation_input_tokens").is_none());
        assert!(v["id"].as_str().unwrap().starts_with("msg_"));
    }

    #[test]
    fn stream_frames_match_the_documented_event_sequence() {
        let start = message_start_body("msg_1", "m");
        assert_eq!(start["type"], "message_start");
        assert_eq!(start["message"]["role"], "assistant");
        assert_eq!(start["message"]["content"].as_array().unwrap().len(), 0);

        let cbs = content_block_start_body();
        assert_eq!(cbs["type"], "content_block_start");
        assert_eq!(cbs["content_block"]["type"], "text");

        let delta = text_delta_body("abc");
        assert_eq!(delta["type"], "content_block_delta");
        assert_eq!(delta["delta"]["type"], "text_delta");
        assert_eq!(delta["delta"]["text"], "abc");

        let u = Usage {
            input_tokens: 5,
            output_tokens: 9,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
            reasoning_output_tokens: None,
        };
        let md = message_delta_body("end_turn", Some(&u));
        assert_eq!(md["type"], "message_delta");
        assert_eq!(md["delta"]["stop_reason"], "end_turn");
        assert_eq!(md["usage"]["output_tokens"], 9);
        assert_eq!(md["usage"]["input_tokens"], 5);
    }

    #[test]
    fn error_envelope_is_the_shape_the_sdk_parses() {
        let v = serde_json::to_value(AnthropicError::authentication()).unwrap();
        assert_eq!(v["type"], "error");
        assert_eq!(v["error"]["type"], "authentication_error");
        assert!(v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("x-api-key"));

        let v = serde_json::to_value(AnthropicError::invalid_request("bad")).unwrap();
        assert_eq!(v["error"]["type"], "invalid_request_error");
        assert_eq!(v["error"]["message"], "bad");
    }

    #[test]
    fn tool_output_preview_never_splits_a_utf8_boundary() {
        let long = "ก".repeat(400); // 3 bytes each — cut lands mid-char
        let v = truncate_preview(&long);
        assert_eq!(v["truncated"], true);
        // The point of the test: this must not have panicked, and the
        // preview must still be valid UTF-8 that round-trips.
        assert!(v["preview"].as_str().unwrap().chars().all(|c| c == 'ก'));
    }
}

/// HTTP-level coverage of the paths that resolve before the agent runs:
/// auth and request validation. A successful call needs a live provider,
/// so it stays out of the unit suite — but these are exactly the
/// responses an SDK has to be able to parse, so their status codes and
/// envelopes are worth pinning.
#[cfg(test)]
mod http_tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn call(
        token_env: Option<&str>,
        headers: &[(&str, &str)],
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let _guard = crate::api_v1::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prior = std::env::var("THCLAWS_API_TOKEN").ok();
        match token_env {
            Some(t) => std::env::set_var("THCLAWS_API_TOKEN", t),
            None => std::env::remove_var("THCLAWS_API_TOKEN"),
        }

        let mut req = Request::builder()
            .method("POST")
            .uri("/v1/messages")
            .header("content-type", "application/json");
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let req = req
            .body(Body::from(body.to_string()))
            .expect("build request");

        let resp = crate::api_v1::router()
            .oneshot(req)
            .await
            .expect("router responds");
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .expect("read body");
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);

        match prior {
            Some(p) => std::env::set_var("THCLAWS_API_TOKEN", p),
            None => std::env::remove_var("THCLAWS_API_TOKEN"),
        }
        (status, json)
    }

    fn minimal() -> serde_json::Value {
        serde_json::json!({
            "model": "m", "max_tokens": 16,
            "messages": [{ "role": "user", "content": "hi" }],
        })
    }

    #[tokio::test]
    async fn the_endpoint_404s_when_the_api_is_disabled() {
        let (status, _) = call(None, &[("x-api-key", "anything")], minimal()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_wrong_token_is_401_in_the_anthropic_envelope() {
        let (status, body) = call(Some("right"), &[("x-api-key", "wrong")], minimal()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["type"], "error");
        assert_eq!(body["error"]["type"], "authentication_error");
    }

    #[tokio::test]
    async fn a_missing_token_is_401_not_a_panic() {
        let (status, body) = call(Some("right"), &[], minimal()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["type"], "authentication_error");
    }

    #[tokio::test]
    async fn bearer_is_accepted_as_well_as_x_api_key() {
        // Both must get PAST auth. They then fail validation on an empty
        // messages array — a 400 rather than a 401 is the proof.
        let empty = serde_json::json!({ "model": "m", "max_tokens": 16, "messages": [] });
        for header in [("x-api-key", "right"), ("authorization", "Bearer right")] {
            let (status, body) = call(Some("right"), &[header], empty.clone()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "header {header:?}");
            assert_eq!(body["error"]["type"], "invalid_request_error");
        }
    }

    #[tokio::test]
    async fn a_body_missing_max_tokens_is_a_400_the_sdk_can_parse() {
        // Not axum's plain-text 422 — the handler deserializes by hand
        // so this stays inside the Anthropic error envelope.
        let body = serde_json::json!({
            "model": "m",
            "messages": [{ "role": "user", "content": "hi" }],
        });
        let (status, json) = call(Some("t"), &[("x-api-key", "t")], body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["type"], "error");
        assert_eq!(json["error"]["type"], "invalid_request_error");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("max_tokens"),
            "message should name the field: {json}"
        );
    }

    #[tokio::test]
    async fn a_conversation_with_no_user_message_is_a_400() {
        let body = serde_json::json!({
            "model": "m", "max_tokens": 16,
            "messages": [{ "role": "assistant", "content": "orphan" }],
        });
        let (status, json) = call(Some("t"), &[("x-api-key", "t")], body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["type"], "invalid_request_error");
    }
}
