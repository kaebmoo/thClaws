# Chapter 20 — Background research (`/research`)

`/research <query>` runs a background job that reads the web and
grows your knowledge base as a **zettelkasten**: one note per idea,
every claim anchored to a verbatim quote from its source, notes linked
to each other and to what the KMS already knew. A second query on a
related topic **updates** the existing notes instead of writing a
parallel set of pages.

## Quick start

```
> /research กฎหมายแรงงานไทย ค่าจ้าง ค่าล่วงเวลา และการเลิกจ้าง
[research started: id=research-523f9c5b] query: กฎหมายแรงงานไทย …
  /research status research-523f9c5b     check progress
  /research show research-523f9c5b       open the map of content
  /research cancel research-523f9c5b     cancel
…
[research done: id=research-523f9c5b → rv2-labour/rv2-labour.md]
```

A typical run: 3 rounds of search + reading, 25–30 sources, 10–12
notes, 3–7 minutes on a fast worker model. The GUI shows a live
"Research" panel on the right edge (phase, round, novelty); the CLI
prints a one-line completion notice above the next prompt.

## What you get

```
<kms>/
├── pages/
│   ├── overtime-pay.md              ← one idea per note (kind: concept)
│   ├── severance-pay.md
│   ├── labour-protection-act-be-2541.md   (kind: entity)
│   ├── thai-labour-law.md           ← map of content for the query (kind: moc)
│   └── …
├── sources/<url-slug>.md            ← cached copy of every cited page
├── runs/2026-09-06-thai-labour-law.md   ← what the run did (rounds, novelty, notes)
└── .research/                       ← digest cache + citation registry (internal)
```

### Notes

Every note is a single entity, concept or claim with a stable slug
that is the idea's own name (`overtime-pay`, `wage-committee`), a
one-sentence abstract on the first line (this becomes the index
summary), two to four short sections, and `[[slug]]` links to related
notes — both the ones from this run and the ones that were already in
the KMS. Frontmatter:

```yaml
type: note
kind: concept          # entity | concept | claim | moc
title: ค่าล่วงเวลา
related: ["employee", "employer", "holiday", "working-day"]
sources: [1, 4, 18, 27]
claims: 8
confidence: 0.93       # mean confidence of the claims used
```

Facts carry `[N]` citations that link to the cached source, and a
generated `## Sources` block lists every cited source. There is no
"research notes" appendix and no verification section: nothing lands
in a note unless it came from a claim that passed the quote check.

### The topic page (map of content)

One note per run, `kind: moc`, slug derived from the query. It is the
research result: a report that **describes the topic** — an opening
answer, then 4–8 sections that synthesise across every claim
(landscape, players, comparison tables when several entities share
attributes, a timeline when claims carry dates, money/policy, trends),
linking each child note inline the first time the story reaches it.
It ends with a `## Map` of every linked note and up to three open
questions. `/research show <id>` opens it.

The plan is made top-down: the worker model first decides how the topic
page should describe the subject, then which things that page must link
to for depth, and only then assigns claims. Child notes are written
after the topic page, with its opening in front of them, so they go
deeper instead of repeating it.

### Claims and the quote check

Each source is read **once** and turned into a digest: the entities it
discusses and up to six claims, each with a **verbatim quote** (≤ 60
characters) from the page. The pipeline checks that the quote really
appears in the fetched text (whitespace- and case-insensitive, Unicode
normalised) and drops any claim that fails. In practice 15–20 % of what
the model proposes is dropped this way — those are exactly the
hallucinated citations a post-hoc verifier used to hunt for, caught
before they can be written. Digests are cached per URL under
`.research/digests/`, so re-running on the same sources costs nothing.

### Which sources a run reads

A search engine ranks for engagement, so a query about a vendor's newest
model returns that vendor's release notes *and* five roundups that
paraphrase them. Before fetching, each round ranks its candidates:

1. **Primary first** — a domain that spells the subject (`deepseek.com`
   for DeepSeek), official docs, and `.gov` / `.edu` / `.ac.*` / `.go.*`.
2. **Reference next** — Wikipedia, arXiv, and major news organisations.
3. **Everything else** after, in the engine's own order.

Nothing is ever excluded; the ranking only decides who is read first
with the round's fetch budget. On top of that, **one domain contributes
at most two pages per round**, so a single site cannot supply half a
run's evidence. The run log's **Evidence** table reports how many
sources of each tier the run read and how many a note ended up citing.

### Citations are stable per KMS

A URL gets one citation number for the life of the knowledge base
(`.research/sources.json`). When a later run merges new claims into an
existing note, the `[N]` markers written last month still resolve.

The same page reached by a different URL is the same source: tracking
parameters (`?utm_source=…`), a `#fragment`, `www.`, `http` vs `https`
and a trailing slash are stripped before lookup, so a run cannot pay
twice for one document or cite it under two numbers.

## Rounds and the novelty stop

1. **Seed round** — search the query, read the top 5 pages, digest them.
2. **Gap rounds** — from the entity and claim tables (never the raw
   pages) the worker model proposes up to 4 new searches: thin entities,
   sub-questions with no claims yet, opposing views, primary sources.
   New URLs are fetched and digested in parallel.
3. **Stop** when a round's *new entities* fall below the novelty
   threshold (default 35 %), when no productive searches remain, or at
   `--max-iter` (default 4). No LLM self-scoring.
4. **Plan** — one call turns the tables plus the list of existing notes
   into a plan, topic page first: its outline and the claims its
   narrative needs, then the notes it links to (`create` or `update`
   per slug, the entities each note covers, why the topic page links
   there). Claims attach to notes by their entity tags — the plan never
   lists claim ids, so it stays small enough to never be cut off — and
   the topic page receives every claim.
5. **Write** — the topic page first, then one call per child note in
   parallel, each from its own claims plus the topic page's opening.
   Updates merge into the existing body (or add a dated `## Update`
   section with `--append`).

## Staying current

Every prompt in the pipeline is anchored to today's date and told that
the model's own memory of "the latest version" is stale — only the
sources decide what is current. The seed round runs a second,
freshness-filtered search (`<query> latest <year>`, past-year results on
Tavily/Brave), every gap round must include at least one "newest
release <year>" query, and any gap query that names the current year or
says latest/newest also goes through the freshness filter. Digests
record each page's publication date; when claims disagree the newer
date wins, and anything that changes over time is written "as of
<date>". Without this a 2026 run on "Chinese AI companies" pinned its
searches to 2025 and reported DeepSeek V3 and Qwen 3.6 as current.

### A run does not lose what it read

Each note body is its own model call, and one failing (a timeout, a
truncated reply) costs that note only — every other note, the sources
and the run log are still written, and the run log's **Warnings**
section names what was skipped. A run only fails outright when no note
could be written at all.

Two runs on one topic on one day are two run logs (`…-topic.md`,
`…-topic-2.md`), so the record of what the earlier one read survives.

## Keeping notes current: `/research refresh`

```
/research refresh <slug>                          refresh one note in the attached KMS
/research refresh <kms> <slug> [<slug>…]          …in a named KMS
/research refresh [<kms>] --all [--older-than 30]  every non-MOC note not updated in N days
```

A refresh uses the note's title as the query, runs a short two-round
search with the freshness filter, and forces the plan to `update` that
note: newer claims are merged in, superseded facts are rewritten as
such, and the note's `updated` date moves. No new map of content is
written. Jobs are queued and run one after another; the Research panel
follows each. The KMS sidebar's page context menu has the same action
("Refresh (research)").

## Slash subcommands

```
/research <query>                            start (v2 pipeline)
/research [flags…] <query>                   start with overrides
/research                                    list all jobs (newest first)
/research status <id>                        phase, round, sources, novelty
/research show <id>                          print the map of content in chat
/research cancel <id>                        cancel; nothing partial is written
/research wait <id>                          block the CLI prompt until done
```

### Flags

| Flag | Default | What it does |
|---|---|---|
| `--kms <name>` | the most recently attached KMS, else a new one named from the query | Target KMS. `--kms new` forces a fresh KMS. The KMS a run writes into is attached to the project automatically, so consecutive queries keep building the same graph. |
| `--lang th\|en\|…` | `th` | Language of note bodies, claim text and titles. Technical terms, model/API names and code stay in English in every language. `--lang query` follows the query's language. |
| `--max-notes N` | 30 | Ceiling on notes per run including the topic page. Thin entities are merged into their parent rather than dropped. **From the slash command the accepted range is 1–20** — the default of 30 and the pipeline's hard ceiling of 50 are only reachable by leaving the flag off. |
| `--min-iter N` / `--max-iter N` | 2 / 4 | Round floor and ceiling. |
| `--novelty 0.X` | 0.35 | Stop when a round adds fewer new entities than this share. `1.0` = always run to `--max-iter`. |
| `--worker-model <id>` | your current model | Model for digests, gap queries, the plan and note bodies. thClaws never switches models on your behalf; name one here if you want research to run on a faster model than your chat model. |
| `--append` | off | Never rewrite an existing note; add a dated `## Update` section instead. For KMSs that need an audit trail. |
| `--dry-run` | off | Read, digest and plan, then write only the run log. Nothing else touches the KMS. |
| `--budget-time 20m` | 25m | Wall-clock ceiling; the job ends as failed past it. |
| `--legacy` | off | The pre-v0.121 page pipeline (`--max-pages`, `--score-threshold`, verify pass). Removed in a later release. |

`--max-pages` is accepted as an alias of `--max-notes`, with the same 1–20 range.

> **A rejected flag value becomes part of your query.** The parser eats
> flags greedily from the front and stops at the first thing it doesn't
> recognise, so that `/research --opinion of the user` researches that
> phrase rather than erroring. The side effect is that a *known* flag
> with an out-of-range or unparseable value — `--max-notes 30`,
> `--novelty 5`, `--max-iter abc` — is not an error either: parsing stops
> there and the flag text is prepended to the query. If a run comes back
> with a topic page about your own flags, that is what happened. Check
> the echoed query on the `[research started: …]` line.

## Why the worker model matters

Digesting and note-writing are mechanical and run in parallel, so
the wall-clock is the latency of one call, not the number of calls.
Measured on this workspace: Gemini 2.5 Flash answers four concurrent
Thai calls in 5–7 s each; DeepSeek v4 Flash queues them about 50 s
apart; a reasoning model takes minutes. On a fast flash model a round
takes 30–40 s; on a serialising or reasoning model the same run needs
15–25 minutes, and the engine logs a hint after the first round. The
pipeline stops adding search rounds once 60 % of the time budget is
spent and writes what it has, so a slow model gives a smaller result,
not a failed one.

## Live progress

**GUI** — the right-edge Research panel shows the phase (`round 2/3:
reading 8 sources`, `planning notes`, `writing 11 notes`), a round bar,
and a per-round history where the bar is the round's novelty.

**CLI** — a completion line above the next prompt:

```
[research done: id=research-523f9c5b → rv2-labour/rv2-labour.md]
[research failed: id=research-x9z8] research time budget exhausted
```

The engine's stderr also logs each round (`[research] round 2: 15
sources, 64 new / 79 (71% novelty), 35.7s`), each digest, the plan and
the note-writing wave, which is the first place to look when a run is
slow.

## Tips

- Consecutive queries land in the same KMS by default (the last one
  attached). Use `--kms new` when you really want a separate graph, and
  `/kms use <name>` to switch which KMS is the default.
- Start with `--dry-run` on a new topic to see the plan before paying
  for notes.
- Use `--append` on a KMS other people read; merges are silent.
- Open the run log under `runs/` to see which searches were made and
  how many claims the quote check dropped.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `research time budget exhausted` | slow model, or many slow sites | check the stderr timings; set `--worker-model`; `--budget-time 40m` |
| digests take 60–90 s each on DeepSeek/Qwen | reasoning tokens on long extraction prompts | nothing to do — research already runs its worker calls at thinking level `off` (see Chapter 6); your chat-level `/thinking` setting is untouched |
| `research found no verifiable claims` | every source was navigation/listing, or the digest model returned nothing | try a more specific query; `--worker-model` |
| a note has `[c:…]` left in the text | the writer emitted a claim id the plan did not include | harmless; edit it out — fixed in current builds |
| the MOC links to a note that does not exist | plan referenced a slug that was dropped for having no claims | links to unknown slugs are turned into plain text in current builds |
| `/research show` opens the run log | the run was `--dry-run` | rerun without it |
