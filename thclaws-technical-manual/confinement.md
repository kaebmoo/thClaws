# Bash confinement

The hard layer under the shell tool. Where the `pre_tool_use` hook
screens a command *string* and `bash.rs` confines only the `cwd`
argument, this module wraps the `sh -c <command>` invocation so **the
operating system** enforces the boundary — writes stay inside the
workspace, secrets stay unreadable — no matter how the command is
written.

That distinction is the whole point. A string screen is a tripwire that
obfuscation defeats:

```sh
$(printf 'r''m') -rf ~/          # string screen: sees nothing to match
eval "$(echo cm0gLXJmIH4v | base64 -d)"
echo pwned > /etc/motd            # bash.rs cwd confinement: not a cwd arg
```

Under confinement all three fail at the kernel, because the process
never had permission to write there.

Source: `crates/core/src/confine.rs`. The gate that decides *whether* a
tool runs at all is [`permissions.md`](permissions.md); this is what
happens to the shell once it has been allowed.

## 1. Backends

| Platform | Mechanism | Notes |
|---|---|---|
| macOS | `sandbox-exec` (Seatbelt) | Allow-by-default profile, then `(deny file-write*)` outside the write roots and `(deny file-read*)` on secrets |
| Linux | **Landlock** | An LSM needing no user namespace, so it works where `bwrap` is AppArmor-blocked. The engine re-execs itself as a hidden `__confine` helper, installs a ruleset, then `exec`s `sh -c` |
| Linux fallback | `bubblewrap` | Used when Landlock is unavailable |
| Anything else | passthrough | Runs unconfined, logged once — never a hard failure |

A confiner is **probed at runtime**, because a binary being present is
not the same as it being usable — a kernel without Landlock, a
container without the right capabilities, a Seatbelt profile the OS
rejects. Probing at spawn rather than trusting `which` is what keeps a
misconfigured host from failing every command.

## 2. Modes

`bash.sandbox` in settings, or `THCLAWS_BASH_SANDBOX` in the
environment — the env var wins, as a CI and power-user escape hatch
(`THCLAWS_BASH_SANDBOX=off`).

| Mode | Write roots |
|---|---|
| `workspace` | **Default.** Workspace + tmp + package-manager caches |
| `strict` | Workspace + tmp only |
| `off` | No confinement |

The mode is process-global — it is a pod-level policy, not a per-call
decision — but the **workspace root is resolved per call** from
`Sandbox::root` / `workdir`, so a multi-tenant session confines to its
own user's folder rather than to whatever the process started in.

### What `workspace` grants beyond `strict`

The package-manager and toolchain caches, so `pip install`, `npm ci`,
`cargo build` and friends work without punching a hole per project:

```
~/.cache  ~/.npm  ~/.pnpm-store  ~/.yarn  ~/.cargo  ~/.rustup
~/.pyenv  ~/.local/share/virtualenvs  ~/.local/share/uv
~/.gradle  ~/.m2
```

`strict` drops all of these. A build under `strict` that needs a cache
will fail — that is the trade, and the reason `workspace` is the
default.

## 3. Always writable, in every confined mode

Temp:

```
$TMPDIR   /tmp   /private/tmp        # macOS resolves /tmp to /private/tmp
```

And a fixed list of character devices, because tools open them
constantly and writing them is not a filesystem escape — `/dev/null`
discards, `/dev/tty` is the terminal the user is already at:

```
/dev/null  /dev/zero  /dev/full  /dev/random  /dev/urandom
/dev/tty   /dev/stdin /dev/stdout /dev/stderr /dev/fd
/dev/ptmx  /dev/dtracehelper        # macOS — some runtimes open it
```

Raw block devices (`/dev/disk*`) are **deliberately not** on that list.

Extra roots come from `bash.sandbox_write_paths`, `~`-expanded.

## 4. Read denial

Both confined modes deny reads of the obvious credential stores,
regardless of write policy — **on macOS Seatbelt and the Linux bwrap
fallback only**:

```
~/.ssh   ~/.aws   ~/.gnupg   ~/.netrc
~/.config/thclaws   ~/.config/gcloud   ~/.config/gh
~/.kube  ~/.docker
```

`~/.config/thclaws` is on that list for a specific reason: it holds the
agent's own secrets. A command the agent runs cannot read the keys the
agent runs on.

Extra entries come from `bash.sandbox_deny_read`.

> **The Landlock path does not enforce this.** `wrap()` tries Landlock
> first and it is what a stock modern Linux takes, so on most Linux
> hosts confinement is **write-only** and the dotfile list above is not
> applied. The source says so at `confine.rs:546` — "the Landlock path
> is *write* confinement only … a future ABI refinement". Do not rely
> on read-denial to keep a secret from a shell command on Linux; use
> file permissions or keep the secret off the box.

## 5. The multiuser read mask

Landlock has no deny rule — only grants — so on a multi-tenant pod
cross-user read protection is expressed as an **allowlist** rather than
a denylist: `read_roots: Some(...)` restricts reads and execs to the
user's own workspace plus system directories.

`/proc` is deliberately excluded from that grant set. Without it,
`/proc/<pid>/environ` is unreadable, which closes credential
exfiltration between same-uid tenants sharing a pod. Single-user
behaviour is unchanged — `read_roots` is `None` there, and the dotfile
denylist above still applies through Seatbelt or bwrap.

See [`multi-tenant-serve.md`](multi-tenant-serve.md) for the rest of
the isolation story.

## 6. Reporting: mode is not the same as enforced

```rust
pub fn enforcement_state() -> (ConfineMode, bool)
```

The second value is `true` only when the mode is not `off`, the
platform has a confiner, **and** no runtime failure was recorded. The
helper signals a failed install by emitting `NO_ENFORCE_SENTINEL` on
stderr, which the parent detects and latches into
`CONFINE_RUNTIME_FAILED`.

This is why the audit record carries **both** values:

```json
"confine": { "mode": "workspace", "enforced": true }
```

An auditor reading `mode: "workspace"` alone would conclude the command
was contained. `enforced: false` says the mode was configured and the
OS did not deliver it — a host missing Landlock, a container without
the capability. Reporting the fact beats inferring it, and it is the
difference between a control and a claim.

A sandbox-denied write appends an actionable hint to the Bash output,
so the model sees why the command failed rather than a bare permission
error.

## 7. Scope

Confinement applies to the Bash tool **and everything it spawns**,
including subagent and workflow shells — the ruleset is installed on
the process before `exec`, so children inherit it. It does not apply to
the engine's own file tools (`Write`, `Edit`), which go through
`Sandbox::check_write` instead; see [`permissions.md`](permissions.md).

## See also

- [`permissions.md`](permissions.md) — the approval gate and the filesystem sandbox this sits under
- [`built-in-tools.md`](built-in-tools.md) — the Bash tool itself
- [`multi-tenant-serve.md`](multi-tenant-serve.md) — why the read mask exists
- [`../thclaws/ENTERPRISE.md`](../thclaws/ENTERPRISE.md) — how `confine.enforced` surfaces in the enterprise audit trail
