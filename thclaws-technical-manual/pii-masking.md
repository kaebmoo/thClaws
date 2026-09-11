# Thai PII masking

Opt-in detection and pseudonymization of Thai personal data **before it
reaches the model**. Detected spans are replaced with `[TYPE_N]`
placeholders on the wire; the real values are restored in the reply on
the way back.

Source: `crates/core/src/sensitive.rs`. Settings: the `sensitive` block
in `settings.json`. Off by default.

## 1. The invariant

**History stays plaintext; the wire carries placeholders.**

That is the whole design in one line, and it is not the obvious choice.
Masking the stored conversation would be simpler, and wrong: a
multi-turn conversation would then accumulate placeholders in its own
history, and any turn that re-sent that history would leak the mapping
the moment one restore went wrong.

Instead the masker runs at the `StreamRequest` build in `agent.rs`, so:

- every turn re-masks from the plaintext original;
- the coreference map keeps `[ID_1]` referring to the same person across
  turns;
- tools, files and the session JSONL only ever see real values.

```
 session JSONL / tools / files          the provider
        (plaintext)                     (placeholders)
              │                               ▲
              │      mask_request()           │
              └───────────────────────────────┘
                     unmask on the way back
```

**Skipped for local providers.** If the model runs on the host —
`ProviderKind::is_local()` — nothing leaves the machine, so masking
would only spend context to protect against nothing.

**Refuses to arm under multiuser.** One shared worker serving several
people cannot keep per-person coreference maps apart, so `configure`
declines rather than risking a cross-tenant mapping. See
[`multi-tenant-serve.md`](multi-tenant-serve.md).

## 2. What it detects

Layer 1 is rules with validators, plus a user-supplied dictionary:

| Type | Placeholder | Rule |
|---|---|---|
| `ThaiId` | `[ID_n]` | 13 digits **with the national-ID checksum verified** |
| `Phone` | `[PHONE_n]` | Thai mobile / landline shapes |
| `Plate` | `[PLATE_n]` | Vehicle registration — **context required** |
| `Name` | `[NAME_n]` | Titled forms only |
| `Custom` | `[CUSTOM_n]` | Anything in the user's dictionary |

Layer 2 (NER for free-form names and addresses) is a separate step and
not shipped here.

## 3. Why precision, not recall, is the constraint

Every span this module returns is **replaced** in the text the model
sees. In English a false positive over-masks one word. In Thai, which
is written without spaces between words, a loose pattern swallows a
whole clause and the prompt arrives mangled — the model then answers a
question nobody asked.

The first cut of these rules produced **119 hits across five PII-free
Thai chapters**. Each detector now carries a boundary or a context
requirement, pinned by `negative_corpus_has_no_detections` against real
prose:

- **`Name` — titled forms only.** `นาย` / `นาง` / `นางสาว` / `น.ส.` / `ด.ช.` / `ด.ญ.`, anchored at a word boundary, with compounds blacklisted. **`คุณ` is deliberately not a trigger**: it is the everyday second-person pronoun and produced 80 hits in the same corpus. Honorific-`คุณ` names are left to Layer-2 NER.
- **`Plate` — context required.** `ทะเบียน…` before, or a province name after. A bare two-to-three consonants plus a number is ordinary Thai ("ครบ 3 ปี").
- **`Phone` / `ThaiId` — rejected when a digit is adjacent**, so the pattern cannot carve a phone number out of a bank-account number.

### Thai numerals are normalized first

`๐๘๑…` is a real phone number and a real ID. Before normalization those
slipped through **un-tokenized** — a fail-open leak, the one failure
mode this module must never have. Digits are folded to ASCII before
matching.

That asymmetry is worth stating plainly: an over-match is a mangled
prompt, an under-match is a leak. The rules are tuned so the mistakes
land on the first side, and the ID checksum means a 13-digit number
that is not a valid Thai ID is left alone.

## 4. Restoring the answer

`unmask` puts real values back. When the output is shown to a person it
is wrapped in markers so it is visible which parts came back from the
map:

```rust
pub const MARK_OPEN:  &str = "🔓« ";
pub const MARK_CLOSE: &str = " »";
```

`Restored` therefore carries two strings:

| Field | Used for |
|---|---|
| `plain` | Tools, files, history, and re-masking on the next turn |
| `display` | What the human reads, with the markers |

Keeping them apart is what stops a marker character from being written
into a file or fed back into the next request.

### Streaming

`feed(&self, pending, chunk)` restores across chunk boundaries. A
placeholder can be split by the stream — `[ID_` in one chunk, `1]` in
the next — so the helper holds back a tail beginning at the last
unclosed `[` (bounded by `MAX_PLACEHOLDER`) and releases it once the
bracket closes. Without that, a split placeholder would reach the user
verbatim.

## 5. Telling the model what it did not see

When values are restored into output, the model is briefed about what
it never saw. Otherwise a model that reasoned over `[NAME_1]` would
describe its own answer inaccurately — it has no way to know the
placeholder stood for something real.

## 6. Configuration

```jsonc
{
  "sensitive": {
    "enabled": true,
    "custom": ["Project Bluebird", "internal-codename"]
  }
}
```

`configure(enabled, custom)` installs the process-wide masker;
`active()` returns it, or `None` when disabled, unarmed, or refused
under multiuser. Per-workspace, via the Settings UI or `settings.json`.

## 7. Limits

- **Layer-2 NER is not shipped.** Free-form names and addresses without a title or context marker are not detected.
- **Thai only.** The detectors are Thai-specific; no English or other-language PII rules.
- **`คุณ`-honorific names are out of scope by design** — see above.
- **Not a compliance control on its own.** It reduces what reaches a provider; it does not make a deployment PDPA-compliant, and it does not touch what tools read from disk.

## See also

- [`agentic-loop.md`](agentic-loop.md) — the `StreamRequest` build where the mask is applied
- [`multi-tenant-serve.md`](multi-tenant-serve.md) — why masking refuses to arm there
- [`providers.md`](providers.md) — `is_local()`, which decides whether masking runs at all
