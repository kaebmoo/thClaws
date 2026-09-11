# Chapter 34 — Managed builds and org policy

Some copies of thClaws are configured by an organisation rather than by
the person using them. This chapter is for the people on the receiving
end: what a managed copy does differently, how to tell whether yours is
one, and what to do when something is refused.

If you installed thClaws yourself and nobody handed you a
`policy.json`, none of this applies — your copy behaves exactly as the
rest of the manual describes.

## The one idea

**A policy file is a gate, not a feature.**

With no policy file, thClaws is the product you have been reading about
for 33 chapters: nothing is enforced, every setting is yours. Present a
**signed** policy file and specific controls switch on. Present a
policy file that fails verification and the binary **refuses to
start** — because falling back silently to the open behaviour would
defeat the entire point of having a policy.

That refusal is the design. A managed copy that quietly ignores a
broken policy is worse than one that won't launch.

## Is my copy managed?

```
❯ /policy status
```

An unmanaged copy says so plainly:

```
no org policy active (open-core defaults)
```

A managed one names the file, the issuer, which key verified it, when
it expires, and which blocks are switched on:

```
policy: /etc/thclaws/policy.json (issuer acme-corp, key embedded)
expires: 2027-01-31T00:00:00Z
branding=on plugins=on gateway=on sso=off
```

## Where the file lives

thClaws looks in three places, in order:

1. `THCLAWS_POLICY_FILE` — an explicit path in the environment
2. `/etc/thclaws/policy.json` — every user on the machine
3. `~/.config/thclaws/policy.json` — just you

The first one found wins. For a managed workstation, `/etc/` is the
usual home: it needs admin rights to write, so a user can't quietly
swap it.

## "thClaws refused to start"

The message you are most likely to meet:

```
thClaws refused to start: this copy is configured for your organization
and its policy file is missing.

Ask whoever provided thClaws for your organization's policy.json, then
save it as one of:
  /etc/thclaws/policy.json          (all users on this machine)
  ~/.config/thclaws/policy.json     (just you)

Nothing else needs installing — the file alone is enough.
```

This is almost always a deleted or never-copied file, not a broken
install. **The file alone is enough** — there is no separate agent,
service, or licence server to set up.

Other refusals — a bad signature, an expired policy, a policy bound to
a different organisation — print what failed and point an end user at
their administrator. If you are testing rather than deployed, removing
the policy file returns the binary to open behaviour.

## What a policy can turn on

Six blocks. Each is independent, and each is off unless the policy says
otherwise.

### `runtime` — the block that says no

The others configure where thClaws *points*. This one restricts what it
may *do*, which is usually the first question an administrator asks.

| Setting | Effect |
|---|---|
| `permission_mode` | Forces `ask`, `auto` or `plan`. Applied **after** settings and after CLI flags — `--permission-mode auto` cannot climb over it |
| `deny_tools` | Named tools are removed from every registry, so the model never even sees them, and refused again at dispatch in case a registry was built somewhere the removal didn't reach |
| `allow_remote` | `false` stops [thClaws Remote](ch33-thclaws-remote.md) — no pairing, no reconnect, no boot autoconnect |
| `allow_serve` | `false` and the binary refuses to bind `--serve` |

The belt-and-braces on `deny_tools` is deliberate: *"the model could
not call it"* is a weaker claim than *"the call does not run"*.

`allow_remote` and `allow_serve` both default to **true**. A policy
closes them by saying so, never by omitting the field — so a partial
policy can't accidentally lock a machine down.

### `branding`

Replaces the product name, logo, support email, and the About text with
the organisation's own. Cosmetic, but it is what makes an internal
rollout feel like an internal tool.

### `plugins`

Restricts where extensions may come from:

- `allowed_hosts` — wildcard host patterns for skills, plugins and MCP
  servers. **An empty list with the block enabled means no external
  sources at all**, which is the air-gapped setting.
- `allow_external_scripts` — `false` (the default) rejects skills that
  ship an executable `scripts/` directory, leaving only declarative
  ones.

### `gateway`

Routes every provider HTTP call through the organisation's own
endpoint. It can be *required* — anything not matching the gateway host
is blocked — or merely *preferred*, which still allows direct provider
access.

This is how an organisation runs thClaws against its own model
deployment without every user configuring keys.

### `sso`

OIDC login, so the person using thClaws is the person your identity
provider says they are. `/sso status`, `/sso login`, `/sso logout`.

### `audit`

Client-side tool-call audit records, emitted to configured sinks. This
is the block that makes a deployment reviewable after the fact.

## The envelope

Beyond the six blocks, the file itself carries:

- **A signature.** Ed25519 over the document. Unsigned or wrongly
  signed means refusal, not a warning.
- **An expiry** (optional). Past it, the policy stops applying — and
  since a managed build requires a policy, that means the copy stops
  working. Administrators should renew before the date, not after.
- **A binding** (optional). An `org_id`, logged at startup so a
  misdeployment shows up in support diagnostics, and optionally a
  binary fingerprint. The fingerprint stops a policy being lifted off
  one build and dropped onto another.

## What this means for you, day to day

- **Settings you change may not take effect.** If `runtime.permission_mode`
  is set, your own choice is overridden after the fact — including
  command-line flags. `/permissions` will show you where you actually
  are.
- **Tools may be missing.** A denied tool is absent from the registry,
  so the model won't offer it and won't mention it.
- **Some features simply refuse**, naming the policy that stopped them.
  That is intentional: being told
  `policies.runtime.allow_remote = false` is more useful than a generic
  error.
- **You cannot override any of it locally.** Editing your own
  `settings.json` doesn't help — the policy is applied last, and it is
  signed.

If a policy blocks something you need for your work, that is a
conversation with whoever issued it, not a configuration problem to
solve.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| "policy file is missing" on launch | The file was deleted or never copied | Get `policy.json` from your administrator; drop it at one of the two paths |
| "refused to start" with a signature error | The file doesn't match the key this build carries | Confirm you were given the file for *this* build |
| Worked yesterday, refuses today | The policy expired | Your administrator needs to issue a renewal |
| A setting keeps reverting | `runtime` is forcing it | `/policy status` shows what's enforced |
| A tool the manual documents isn't there | `deny_tools` | Same |
| `/policy status` says open-core, but IT says it's managed | The file isn't in a path thClaws searches | Check the three locations above, in order |

## See also

- [Chapter 5](ch05-permissions.md) — permission modes, which
  `runtime.permission_mode` overrides.
- [Chapter 33](ch33-thclaws-remote.md) — Remote, which
  `runtime.allow_remote` closes.
- [Chapter 3](ch03-working-directory-and-modes.md) — `--serve`, which
  `runtime.allow_serve` closes.
- `ENTERPRISE.md` in the thClaws distribution — the administrator-facing
  reference: the full file format, how policies are signed and issued,
  and how to build a managed copy.
