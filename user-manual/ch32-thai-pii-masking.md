# Chapter 32 — Thai PII masking

Swap Thai personal data for placeholders **before the text leaves your
machine**, and put the real values back before you see the reply. The
model works on `[PHONE_1]` and `[ID_2]`; your provider's logs never see
the digits.

**Off by default.** Turn it on in `.thclaws/settings.json`:

```json
{ "sensitive": { "enabled": true } }
```

There is no GUI toggle and no slash command — this is a settings-file
decision, made once per project.

## What it looks like

You type a message with a phone number in it. The model receives:

```
ผู้ติดต่อคือ [PHONE_1] โปรดร่างอีเมลถึงเขา
```

The model answers using the placeholder. Before that answer reaches
you, the real value is restored and marked so you can see what
happened:

```
ผมจะร่างอีเมลถึง 🔓« 081-234-5678 » ให้ครับ
```

The 🔓« » decoration is display-only. Anything the machine consumes —
files written, tools called, the session history — gets the plain
restored value with no markers in it.

## The rule that makes it safe

**History stays plaintext; the masking lives at the wire.**

That is the invariant worth understanding, because it is the opposite
of what you might expect. Your session file on disk holds the real
values. The masking happens when the request is assembled, on every
turn, freshly.

Doing it the other way — masking into history — sounds tidier and is
broken: a placeholder written into history in turn 3 has to keep
meaning the same thing in turn 30, across compaction, across `/load`,
across a model switch. One missed re-map and the model is reasoning
about the wrong person. Masking at the wire means each turn re-masks
from the real text, and the coreference map keeps `[PHONE_1]` pointing
at the same number for the life of the session.

## What gets masked

Four built-in detectors, each with a validator rather than a bare
pattern:

| Type | Placeholder | How it's recognised |
|---|---|---|
| Thai national ID | `[ID_1]` | Any 13-digit run, then the **official checksum** decides. A 13-digit number that isn't a valid ID is left alone |
| Phone | `[PHONE_1]` | Thai phone shapes, rejected when a digit sits adjacent — so it can't carve a phone out of a bank-account number |
| Licence plate | `[PLATE_1]` | Requires context: `ทะเบียน…` before it, or a province name after |
| Titled name | `[NAME_1]` | `นาย` / `นาง` / `นางสาว` / `น.ส.` / `ด.ช.` / `ด.ญ.` followed by a name |

**Thai numerals are normalised first.** `๐๘๑…` is a real phone number,
and an early version let it through un-masked. That is the one failure
mode this feature must never have — leaking because the pattern didn't
recognise the digits — so `๐-๙` is converted to ASCII before matching.

Everything text-carrying in the request is covered: your message, the
system prompt, tool arguments, and tool results.

## Precision is the constraint, not recall

This is the design decision that explains the rest of the chapter.

Thai is written without word spaces. A loose pattern doesn't just
over-mask a word — it swallows the clause around it, and the model
receives a mangled prompt. So the detectors are deliberately narrow,
and each carries a boundary or a context requirement.

The first version of these rules produced **119 hits on five Thai
chapters containing no PII at all**. That's what the tuning was for.

The clearest example: **`คุณ` is not a name trigger.** It is the
everyday second-person pronoun — "you" — and it appeared 80 times in
that same PII-free corpus. Treating `คุณสมชาย` as a name would mean
treating half of ordinary Thai conversation as personal data. So
honorific-`คุณ` names are left uncaught on purpose.

Similarly, a bare two-or-three consonants followed by a number looks
like a plate but is also ordinary Thai (`ครบ 3 ปี` — "3 years
complete"), so plates need context.

## What it does not catch

Be clear-eyed about this before relying on it:

- **Names without a formal title.** `คุณสมชาย`, or just `สมชาย`. This
  is the biggest gap, and it is deliberate — see above. Free-form name
  and address detection needs a language model rather than a regex, and
  that layer is planned, not shipped.
- **Addresses.** Same reason.
- **Email addresses and bank accounts.** No detector today.
- **Faces and documents in images.** A photo of an ID card is real
  personal data that no text rule can touch. Images pass through
  untouched.
- **Anything outside Thai patterns.** This is a Thai-specific feature.

Use the placeholder mechanism as a strong reduction in exposure, not as
a compliance guarantee. If a document must never reach a provider, do
not paste it into a chat that goes to one.

## Where it does not run

Two cases turn masking off entirely, both on purpose:

- **Local models.** Ollama, LM Studio, vLLM and llama.cpp run on your
  machine — nothing leaves the host, so masking would only cost the
  model context it could have used. It is skipped per request, based on
  the model actually being used for that turn.
- **Multiuser pods.** Masking refuses to arm when the engine is running
  multi-tenant. One shared worker serves several people, and the
  coreference map is per process — the failure mode would be one
  tenant's `[PHONE_1]` resolving to another tenant's number. Refusing
  is the only safe answer.

**Thinking blocks are also left alone.** A model's reasoning is
placeholder-only by construction — it only ever saw placeholders — and
Anthropic signs those blocks, so rewriting them would break the
signature.

## What the model is told

While masking is armed, the system prompt gains a **Redacted values**
briefing. It exists because the obvious failure is conversational, not
technical: a model that sees `[PHONE_1]` and reads it as *missing* will
politely ask you to send the real number — and the un-masker will then
rewrite that sentence into a claim that the real number is a
placeholder.

So the model is told, in short: a placeholder stands for real data the
user did provide; it is not missing, not a template, not an error;
reuse it verbatim and the real value is substituted back before any
tool runs; and don't try to validate or reformat it, because you cannot
see the underlying digits.

## Custom terms

The detector supports a custom dictionary — project-specific terms
masked as `[CUST_1]`. **There is no settings key for it yet**, so today
it is only reachable from code. If you need a specific string masked,
that is a gap, not a configuration you're missing.

## Turning it on

```json
{
  "sensitive": { "enabled": true }
}
```

Per project, in `.thclaws/settings.json`. Changing it takes effect for
new requests; an already-armed session keeps its coreference map, so an
unrelated settings change mid-conversation will not renumber your
placeholders.

Remember that a malformed `settings.json` is [parsed
silently](ch05-permissions.md) — every opt-in flag defaults off with no
warning. If you set this and see no placeholders, check the file is
valid JSON first.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Nothing is masked | Not enabled, or `settings.json` doesn't parse | Verify the JSON; `sensitive.enabled` must be `true` |
| Nothing is masked, but the setting is right | You're on a local model, or a multiuser pod | Expected — see above |
| A name went through | It had no formal title, or used `คุณ` | Expected today; free-name detection is not shipped |
| A 13-digit number was not masked | It failed the Thai-ID checksum, so it isn't an ID | Working as intended |
| The model asked me to "send the real number" | It read the placeholder as missing | The briefing exists to prevent this; report it if it recurs |
| I see 🔓« » in a file the agent wrote | Would be a bug — the decoration is display-only | Report it |

## See also

- [Chapter 5](ch05-permissions.md) — permissions, and the silent
  `settings.json` parse failure.
- [Chapter 6](ch06-providers-models-api-keys.md) — which providers are
  local (and therefore skip masking).
- [Chapter 27](ch27-thclaws-cloud.md) — what the cloud gateway does and
  does not see.
