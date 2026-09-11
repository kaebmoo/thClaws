# Enterprise policy and audit — code map

The full reference for what a policy *does* is
[**`ENTERPRISE.md`**](../thclaws/ENTERPRISE.md) (Thai:
[`ENTERPRISE-th.md`](../thclaws/ENTERPRISE-th.md)) — the administrator
guide, kept current because it ships to customers. This page is the
engineering complement: where the code lives, where it is consulted, and
the rules you have to follow when adding to it. It deliberately does not
restate the policy schema.

Source: `crates/core/src/policy/` (2,088 lines) and
`crates/core/src/audit/` (911).

## 1. The architectural rule

**Every enterprise control is open-core code gated by a signed policy.**
No feature flags, no licence checks, no closed branch. A control is
added by

1. extending the policy schema,
2. reading `crate::policy::active()` at the decision point,
3. leaving behaviour **byte-identical** when the block is absent.

Point 3 is not a style preference. It is the promise that lets the same
binary serve the community and a bank, and it is why you will not find
`#[cfg(feature = "enterprise")]` anywhere. If you reach for one, the
design has gone wrong.

Customer-specific material is configuration — the policy, branding
assets, private plugins — never code.

## 2. Code map

| Area | File | Holds |
|---|---|---|
| Schema + loader | `policy/mod.rs` | `Policy` / `Policies` and the per-block structs, `ActivePolicy`, `active()`, `load_or_refuse()`, the search path, `validate_policies()`, `status_text()` |
| Verification | `policy/verify.rs` | Ed25519 verify, canonical JSON (sorted keys, no whitespace, no external dependency), `KeySource` resolution, binding and expiry checks |
| Errors | `policy/error.rs` | `PolicyError` — every variant fail-closed — and `refuse_message()` |
| Allow-list | `policy/allowlist.rs` | The host + path glob matcher and `AllowDecision` |
| Audit core | `audit/mod.rs` | `init`, `set_session`, `begin_turn`, `record_tool_call`, `record_denied`, `shutdown`, `correlation_headers`, `status_line` |
| Record shape | `audit/record.rs` + `record.v1.schema.json` | The `audit.v1` record, serialized in schema key order |
| Sinks | `audit/sink.rs`, `file.rs`, `http.rs` | The `AuditSink` trait, the JSONL file sink, the batched NDJSON HTTP sink |
| Build embed | `crates/core/build.rs` | Public key and optional embedded policy → `THCLAWS_EMBEDDED_POLICY_*` |
| Operator CLI | `src/bin/policy_tool.rs` | keygen / sign / verify / inspect / fingerprint. **Signing exists only here** — the runtime has none |

## 3. Where `policy::active()` is consulted

Knowing these is the difference between adding a control and adding a
suggestion:

- `repl.rs::build_provider` — gateway redirect, at the top of the function.
- `plugins.rs::install`, `skills.rs` (four call sites), `config.rs::parse_mcp_json` — the allow-list, script and MCP gates.
- `prompts.rs::load`, the REPL banner, the GUI title — branding.
- `sso::*` — reads `policies.sso`; `gateway::render_template` pulls `{{sso_token}}`.
- `audit::build()` — reads `policies.audit` once at startup.
- `agent.rs` tool dispatch, **two arms** (the concurrent fast path and the sequential path) — `record_tool_call` / `record_denied`. MCP and workflow tools pass through the same arms, so there is no separate MCP hook.
- `session.rs` and `shared_session.rs` — `audit::set_session`.
- Exit paths (`repl.rs` ×2, `gui.rs`, both binaries' print mode) — `audit::shutdown()`.

## 4. Adding a policy block

1. **Schema.** `pub struct XPolicy` with `#[serde(default)]` fields, and `pub x: Option<XPolicy>` on `Policies` with `#[serde(default, skip_serializing_if = "Option::is_none")]`. The skip is load-bearing: it keeps the canonical bytes of already-signed policies unchanged, so they still verify.
2. **Validation.** Extend `validate_policies()` for "enabled but unusable" combinations — an enabled gateway with no URL, enabled audit with no sinks. Test the refusal *and* the disabled-passes case.
3. **Enforcement.** Read the accessor at the decision point. An absent block must take exactly the old code path. Prefer one choke point to wrapping many call sites — the audit block found a single dispatch site where the RFC had assumed four.
4. **Surface.** Add it to `/policy status`, and make the error name the field when the policy blocks something.
5. **Tamper thinking.** Ask whether `settings.json`, an environment variable, a CLI flag or a hook can switch it off. If any can, it is not an enterprise control. `runtime.permission_mode` is applied *after* both settings and CLI flags for exactly this reason.
6. **Docs.** `docs/enterprise/` first (workspace-only SSOT), then propagate to `ENTERPRISE.md` and its Thai translation.

## 5. Audit internals worth knowing

- **Records are validated against the schema in tests.** `record.v1.schema.json` is `include_str!`-ed and every emitted shape is checked. An additive change needs a schema edit plus a test; a breaking one needs `v: 2`.
- **Sinks are fail-open by contract.** `AuditSink { name, write, flush, dropped_extra }`; the registry counts write failures and `HttpSink` adds failed-batch drops through `dropped_extra`. A dead SIEM never blocks a tool call — the counts surface in `/policy status` and in the `session_end` record.
- **Turn counter is global and advanced only by the `Main` agent origin**, so subagent and side-channel calls attribute to the user turn that spawned them.
- **Actor resolution order:** multiuser member id → SSO session e-mail → `USER` / `USERNAME`.
- **Summaries come from the tool, never from the audit layer.** `Tool::audit_summary(&input) -> Option<AuditSummary>` is implemented for Bash, Write, Edit, Read and MCP. Do not derive a summary by regex over tool input in the audit layer — the tool knows what its own arguments mean.
- **`confine::enforcement_state()` supplies `{mode, enforced}`.** See [`confinement.md`](confinement.md) for why both are recorded.

## 6. Publishing rules

- `thclaws/crates/**`, `frontend/**`, the manuals, `ENTERPRISE.md` and `ENTERPRISE-th.md` are rsynced to the public mirror at release. **Enterprise code is therefore public.**
- `docs/**` — the internal enterprise SSOT — is **never** synced.
- Customer keys and policies live in `thclaws-config/`; `policy.key` is gitignored.

## See also

- [**`ENTERPRISE.md`**](../thclaws/ENTERPRISE.md) — the administrator reference: policy format, every block, deployment, rotation, troubleshooting
- [`confinement.md`](confinement.md) — `confine.enforced`, which the audit record carries
- [`sso.md`](sso.md) — the OIDC flow behind the `sso` block
- [`provider-gateway.md`](provider-gateway.md) — how the `gateway` block substitutes the provider
- [`permissions.md`](permissions.md) — the gate `runtime.permission_mode` overrides
