# thClaws Remote

A local engine reachable from the cloud, with **no public IP and no
inbound port**. The desktop dials *out* to a relay and holds the
connection open; the cloud dashboard's "talk to my local agent" — or any
registered inbound adapter — is then routed down that tunnel.

> **Naming.** The product is **thClaws Remote**. The code is
> `phone_home` — the module, the `/ph/*` relay routes and the `ph:` keys
> all keep that name, and renaming them would break live tunnels and
> force every user to re-pair. Expect both names; they are the same
> feature.

Source: `crates/core/src/phone_home/` — `config.rs` (the on-disk
binding), `mod.rs` (client construction and pairing), `bootstrap.rs`
(lifetime), `session.rs` (the inbound sink). `bootstrap` and `session`
are gui-gated: they forward into the `shared_session` worker, which the
CLI binary does not have.

## 1. What makes it different from the chat bridges

LINE, Telegram and Messenger bind the engine to a **messaging-platform
user id**, paired with an 8-character code. Remote binds it to a
**thClaws.cloud account**.

That changes the trust story. There is no third-party platform in the
middle holding the identity — the binding JWT is minted from the user's
own cloud login, so the account that owns the tunnel is the account that
owns the agent.

Everything else is shared: the same [`bridge`] transport, the same relay
host (`line.thclaws.ai`, which carries the `/ph/*` routes alongside the
messaging ones), and the same inbound-envelope shape.

## 2. Pairing

```
  thclaws.cloud CLI token
          │  POST /ph/pair
          ▼
      relay mints a binding JWT
          │
          ▼
  .thclaws/state/phone-home.json
```

`phone_home::pair` exchanges a cloud CLI token for a binding JWT and
persists it. The file holds the JWT plus an optional relay override, so
later launches auto-reconnect without another login.

There is **no `/remote` slash command** — pairing is a GUI action,
dispatched over IPC as `phone_home_pair`, with `phone_home_connect`
reconnecting an existing binding. The worker does the exchange
(`shared_session.rs`); the REPL has no entry point, matching the
gui-gating of the rest of the module.

The token is sent as `?token=` on the WebSocket and as `Bearer` on
replies. The relay default is `https://line.thclaws.ai`, overridable
with `THCLAWS_PHONE_HOME_SERVER` for development.

## 3. The tunnel

`build_client` wraps the generic `BridgeClient` with the `phone-home`
tag and a shared `CancelToken`. `bootstrap::spawn` owns its lifetime and
hands back a `PhoneHomeHandle`:

```rust
pub struct PhoneHomeHandle {
    pub cancel: CancelToken,
    pub join: tokio::task::JoinHandle<()>,
    pub server_url: String,   // no token — safe to show in the UI
    …
}
```

**Dropping the handle does not stop the tunnel** — fire
`cancel.cancel()` first. The `server_url` field deliberately excludes
the token so the UI can display where the tunnel points without
rendering a credential.

## 4. Inbound: one direction only

`PhoneHomeSink` implements `BridgeEnvelopeSink`. Each inbound
`WsEnvelope::UserMessage` is injected into the worker as
`ShellInput::LineMessage` to drive a turn — the same path LINE and
Messenger use.

The reply is **not** posted back through the sink. The oneshot it
creates is deliberately ignored, because the worker's `ViewEvent`
fan-out already streams the whole turn — the user echo, assistant
deltas, `turn_done` — up `/ph/event`. The dashboard therefore renders a
remote turn live, exactly like the local GUI, instead of waiting for one
final block of text.

That asymmetry is the design: inbound goes through the sink, outbound
goes through the existing event fan-out. There is no second rendering
path to keep in sync.

## 5. Enterprise: switching it off

Remote is the surface a regulated deployment worries about most — it
makes a workstation's agent reachable from outside the network. The
`runtime` policy block closes it:

```json
"runtime": { "enabled": true, "allow_remote": false }
```

Every path that starts a session refuses: boot autoconnect, the
`phone_home_pair` IPC action, and reconnect. `bootstrap.rs` prints the reason naming
`policies.runtime.allow_remote = false`, so a user who wonders why the
tunnel will not come up is told which policy stopped it rather than
seeing a generic failure.

The default is `true` — a policy can only close Remote by saying so, not
by omitting a field. See [`../thclaws/ENTERPRISE.md`](../thclaws/ENTERPRISE.md).

## 6. Limits

- **GUI only.** `bootstrap` and `session` forward into the `shared_session` worker; `thclaws-cli` has no worker to forward into.
- **One binding per project.** `.thclaws/state/phone-home.json` is project-scoped, so a second workspace pairs separately.
- **The relay is ours.** A self-hosted relay is not a supported configuration today; `THCLAWS_PHONE_HOME_SERVER` exists for development.

## See also

- [`line-bridge.md`](line-bridge.md) — the shared relay and transport
- [`multi-tenant-serve.md`](multi-tenant-serve.md) — the other way to reach an engine remotely
- [`../thclaws/ENTERPRISE.md`](../thclaws/ENTERPRISE.md) — `runtime.allow_remote`
