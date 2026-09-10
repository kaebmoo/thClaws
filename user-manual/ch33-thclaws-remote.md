# Chapter 33 — thClaws Remote

Reach the agent on your own machine from anywhere, with **no public IP
and no open port**. Your desktop dials *out* to a relay and holds that
connection open; `thclaws.cloud/talk` in any browser then routes your
messages down the tunnel to the engine running on your laptop, and the
replies stream back up it.

The work still happens locally. Your files, your keys, your tools, your
model configuration — none of it moves. Only the chat text travels.

> **Two names for one feature.** The product is **thClaws Remote**. The
> code calls it **phone-home** — the module, the relay's `/ph/*` routes,
> and the binding file on disk all use that name, and renaming them
> would break every live tunnel and force everyone to re-pair. Expect
> to see both; they are the same thing.

## Why not one of the other routes?

thClaws has several ways to be reached from elsewhere, and they answer
different questions:

| You want | Use |
|---|---|
| To chat with *your own* machine's agent from a browser | **thClaws Remote** (this chapter) |
| A phone-first chat surface, with approvals as tappable chips | [Telegram](ch23-telegram.md) or [LINE](ch21-line-and-browser-chat.md) |
| An HTTP API other programs call | `--serve` ([Chapter 3](ch03-working-directory-and-modes.md)) |
| An agent that runs when your laptop is closed | A [hosted workspace](ch27-thclaws-cloud.md) — a different machine entirely |

The distinction that matters most is the last one. Remote does **not**
run anything in the cloud. If your laptop is asleep, there is nothing
at the other end of the tunnel and the browser will tell you so.

The other difference is identity. LINE, Telegram and Messenger bind the
engine to a **messaging-platform account** — a LINE user id, a Telegram
user id. Remote binds it to your **thClaws.cloud account**. There is no
third-party platform in the middle holding the identity, so the account
that owns the tunnel is the account that owns the agent.

## Turning it on

You need a thClaws.cloud CLI token saved first — Remote is minted from
your cloud login, so without it the button stays disabled. See
[Chapter 27](ch27-thclaws-cloud.md#setting-the-catalog-url--a-cli-token)
for how to save one.

Then, in the desktop GUI:

1. Open **Settings → thClaws.cloud**.
2. Scroll to **📡 thClaws Remote**.
3. Click **Enable**.

The button reads *Enabling…*, the desktop exchanges your CLI token with
the relay for a binding token, saves it, and connects. If you never
saved a cloud token, hovering the button says so: *"Save a thClaws.cloud
CLI token above first."*

That is the whole setup. There is no code to type on the other end and
no `/remote` slash command — pairing is a GUI action, deliberately, and
the CLI has no equivalent (see [Limits](#limits)).

## Talking to it

Open **[thclaws.cloud/talk](https://thclaws.cloud/talk)** in any
browser, signed in to the same account. Type; the message rides the
tunnel to your desktop, the agent runs the turn locally with its full
tool registry, and the reply streams back token by token.

### "Connected" means two different things

The page tracks two separate connections, and it is worth knowing which
one a problem is in:

- **The browser ↔ relay socket.** This is what "connecting / open /
  closed" refers to. It says nothing about your laptop.
- **Your desktop's presence at the relay.** Reported separately. The
  browser being connected does **not** mean the desktop is.

So "connected, but nothing answers" is the normal shape of *your laptop
is asleep or offline*, not a bug. Wake the machine and the tunnel
re-establishes itself.

## The binding, and where it lives

Pairing writes a small file into the project you paired from:

```
.thclaws/state/phone-home.json
```

It holds the binding token, an optional relay override, and a machine
label that shows up in the cloud device list (cosmetic — the binding is
keyed by the token, not the label). Later launches read it and
reconnect on their own, so you enable Remote once per project and then
forget about it.

**The binding is per project**, because the file is. A second workspace
on the same machine is a separate pairing — enable it there too. That
is a consequence of the file living under `.thclaws/state/`, not a
policy decision, but it has a useful side effect: a folder you never
paired can never be reached, however many other folders you did pair.

A malformed `phone-home.json` is treated as "not paired" rather than an
error, so a corrupted file makes the Enable button work again instead
of wedging the app.

## Disconnecting

Two different things you might mean:

- **Stop the tunnel for now.** Quit thClaws, or disconnect from the
  GUI. The binding stays on disk and the next launch reconnects.
- **Un-pair this folder.** Delete `.thclaws/state/phone-home.json`. The
  next launch has nothing to reconnect with, and `/talk` reports your
  desktop as offline.

## Security and trust boundary

- **Nothing is listening on your machine.** The tunnel is an outbound
  connection your desktop opens. There is no port to expose, no
  firewall rule to add, and nothing for a scanner to find.
- **The relay carries chat text.** Messages pass through
  `line.thclaws.ai` — the same relay that serves the LINE and Messenger
  bridges — to be routed. Your prompts to Anthropic / OpenAI / etc. do
  **not** go through it; those run desktop → provider directly.
- **Your keys never leave the desktop.** The relay holds the binding
  token and nothing else. It cannot read your provider keys, your
  files, or your KMS.
- **Whoever holds your cloud account holds the tunnel.** That is the
  point of binding to a cloud login rather than a chat platform — and
  it is also the thing to protect. Treat cloud-account access as
  equivalent to sitting at your desktop.
- **Approvals still apply.** A Remote turn runs under the same
  permission mode as any other ([Chapter 5](ch05-permissions.md)). If
  you are in `auto`, a message typed from a café runs tools without
  asking.

## Turning it off for an organisation

Remote is the surface a regulated deployment worries about most: it
makes a workstation's agent reachable from outside the network. The
managed-policy `runtime` block closes it:

```json
"runtime": { "enabled": true, "allow_remote": false }
```

Every path that would start a tunnel then refuses — boot auto-connect,
the Enable button, and reconnect — and the engine prints the reason,
naming `policies.runtime.allow_remote = false`, so a user who wonders
why it will not come up is told which policy stopped them rather than
seeing a generic failure.

The default is `true`: a policy can only close Remote by saying so, not
by omitting the field. Chapter 34 covers managed builds and the policy
file in full.

## Limits

- **GUI only.** The tunnel forwards into the shared-session worker,
  which `thclaws-cli` doesn't have. `thclaws --cli` cannot pair or
  serve a Remote session.
- **One binding per project**, as above.
- **The relay is ours.** Self-hosting the relay is not a supported
  configuration today. `THCLAWS_PHONE_HOME_SERVER` exists to point a
  development build at a different relay, not as a deployment option.
- **No offline queue.** A message sent while the desktop is down is not
  held for later.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| **Enable** button is greyed out | No thClaws.cloud CLI token saved | Settings → thClaws.cloud → paste a token, Save, then Enable |
| "log in to thClaws.cloud first" | Same — the token wasn't readable at pair time | As above |
| `/talk` connects but nothing answers | Your desktop is asleep, offline, or thClaws isn't running | Wake the machine and open thClaws; the tunnel re-establishes itself |
| Paired on one project, nothing on another | Bindings are per project | Enable Remote from that folder too |
| Refused with a policy message | `policies.runtime.allow_remote = false` | Ask whoever manages the policy; it can't be overridden locally |
| Enabled it, but `thclaws --cli` won't pair | Expected — GUI only | Pair from the desktop app |

## See also

- [Chapter 27](ch27-thclaws-cloud.md) — the cloud account, CLI tokens,
  and hosted workspaces (the "runs without your laptop" alternative).
- [Chapter 21](ch21-line-and-browser-chat.md) and
  [Chapter 23](ch23-telegram.md) — the chat-platform bridges, which
  share this relay and transport.
- [Chapter 5](ch05-permissions.md) — what a remote turn is allowed to
  do.
- Technical manual: `thclaws-remote.md` for the tunnel internals.
