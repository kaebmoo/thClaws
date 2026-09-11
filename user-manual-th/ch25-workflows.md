# บทที่ 25 — Workflows

Workflows คือ **orchestration tier ที่สี่** ของ thClaws — model
เขียนสคริปต์ JavaScript ที่กระจายงานไปยัง subagent หลายตัว แล้ว JS
engine ในตัวสคริปต์รันแบบ deterministic บนเครื่องของคุณ ต่างจาก
subagent (บทที่ 15), `/agent` side-channel, หรือ Agent Teams (บทที่
17) ตรงที่ตัวสั่งการคือ **code** ไม่ใช่ model — ซึ่งหมายความว่ารัน
workflow เดิมซ้ำจะได้รูปทรงงานเหมือนเดิมทุกครั้ง และงานยาว ๆ จะ
เหลือ checkpoint ไว้บนดิสก์

Fan-out, JSON-schema validation, budget token/time ต่อ worker, retry,
KMS-write grant, และ resume ใช้งานได้หมดแล้ว ช่องว่างหลักที่ยังเหลือ
คือ **การรันขนานจริง** — worker ยังรันทีละตัวแม้อยู่ใน `Promise.all`
(ดู "ข้อจำกัดปัจจุบัน" ด้านล่าง)

## ควรใช้ workflows เมื่อไร

ใช้ workflows กับ **งาน bulk ที่อิสระจากกันและต้องการความแน่นอน**:

- "rewrite test file 800 ไฟล์ให้ใช้ fixture ใหม่"
- "แปลทุก `.md` ใต้ `kms/bug/` เป็นภาษาไทย"
- "audit `Cargo.toml` ของแต่ละ crate แล้ว flag deps ที่ deprecated"

ใช้ `Task` tool (บทที่ 15) กับ **side-quest ที่ model ตัดสินใจสร้าง
ขึ้นมาเอง** กลางเทิร์น — นั่นคือสิ่งที่ subagent ทำต่อไป

ใช้ `/agent` (บทที่ 15) เมื่อ **คุณ** รู้ชัดว่าจะให้ specialist ทำ
อะไร และอยากให้ทำงานคู่ขนานกับ session หลัก

ใช้ Agent Teams (บทที่ 17) เมื่อ teammate ต้อง **ร่วมมือกัน** —
แลก message ถกเถียงสมมติฐาน ประสานงานบน task list ร่วมกัน
Workflows เป็น stateless fan-out ส่วน team เป็น stateful collaboration

## สองทางในการเรียก workflow

Engine เดียวกัน (LLM เขียนสคริปต์ JS แล้ว Boa sandbox รัน) แต่มี
ทางเข้า 2 ทาง ขึ้นกับว่าใครเป็นคนตัดสินว่าควรใช้ workflow:

| Trigger | เหมาะกับ | UX |
|---|---|---|
| **คุณพิมพ์ `/workflow run <prompt>`** (slash command) | คุณรู้อยู่แล้วว่าควร fan-out และอยากเห็นสคริปต์ก่อนรัน | Author → review → approve/cancel/re-author loop → run |
| **Model เรียก tool `WorkflowRun`** | คุณอธิบายงานเป็นภาษาธรรมชาติ ("rewrite all tests ให้ใช้ fixture ใหม่") แล้วให้ model ตัดสินเองว่าจะ spawn subagent ตัวเดียวหรือ author workflow ขนาน | approval prompt ต่อ call (เหมือน Bash) → author → run, ไม่มี review loop |

เลือก slash command เมื่ออยากเห็น JS ก่อนรัน — ดีกับ pattern ใหม่
หรือเวลาที่กำลังลองรูปแบบงาน เลือกเส้น tool เมื่ออยากได้ผลลัพธ์
เลยและไว้ใจให้ model เลือก orchestration strategy เอง

Model รู้จัก `WorkflowRun` เพราะอยู่ใน section **Collaboration
primitives** ของ system prompt คู่กับ Subagent (side-quest ครั้งเดียว)
กับ Agent Teams (collaborator ที่อยู่นาน) — เลือก primitive ที่
เหมาะสมได้เองโดยไม่ต้องบอก ถ้าอยาก nudge ชัด ๆ บอกใน chat ได้เลย
"ใช้ WorkflowRun ทำ …"

ทั้งสองทาง reject nested call: สคริปต์ที่พยายามเรียก `WorkflowRun`
ในตัวเองจะ fail พร้อม error ชัดเจน — orchestrate ผ่าน
`thclaws.subagent(...)` (แบบเรียงลำดับ) หรือ `thclaws.parallel([...])`
(แบบขนานจริง) ในสคริปต์แทน

**รัน workflow ที่เขียนไว้แล้วพร้อม input แบบมีโครงสร้าง** ship ไฟล์ `.js`
ไปกับ agent ของคุณแล้วรันตรง ๆ โดยไม่ต้อง author ใหม่

```
WorkflowRun({ script_path: ".thclaws/state/workflows/research.js",
              args: { query: "AI agent frameworks", kms: "ai-agents", min_iter: 2 } })
```

สคริปต์อ่าน `args` ได้ตรง ๆ (เช่น `const q = args.query`) แทนแพตเทิร์นเดิม
ที่ต้องเขียนโจทย์ลงไฟล์ `.thclaws/TASK.md` แล้วให้สคริปต์ไป parse เอง

> **หมายเหตุเรื่อง surface** `thclaws.subagent(...)` ใน workflow ต้องใช้ tool
> `Task` ซึ่งลงทะเบียนไว้บน `--cli`, `--serve` และ GUI — **ไม่มี** บน `-p`
> (print) และ `/v1` API บน surface พวกนั้นการเรียก subagent จะ fail
> พร้อม error ที่ชัดเจน แทนที่จะคืน stub เงียบ ๆ ให้ทดสอบ workflow บน
> surface ที่มี subagent จริง

## เริ่มใช้

```text
/workflow run summarize each .rs file under src/ in one line
```

ลำดับเหตุการณ์:

1. **Author phase** model เขียนสคริปต์ JavaScript ที่ใช้ API
   `thclaws.*` (รายละเอียด API อยู่ใน system prompt ของ model
   อยู่แล้ว ดังนั้นสคริปต์ที่ได้กลับมารู้ว่ามีอะไรให้ใช้บ้าง)
2. **Review** สคริปต์ถูก print พร้อมเลขบรรทัด แล้วถาม:
   ```text
   [a]pprove · [c]ancel · [r]e-author:
   ```
   - `a` — รันตามนี้
   - `c` — ยกเลิก
   - `r` — ใส่ note บรรทัดเดียวบอกว่าให้แก้อะไร ("ใช้ read tool ไม่
     ใช่ bash cat") แล้ว model เขียนสคริปต์ใหม่ตาม feedback วน
     จนกว่าจะกด `a` หรือ `c`
3. **Execute** แสดง workflow id (`wf-…`) จากนั้นทุก subagent call
   จะมีบรรทัด progress:
   ```text
   ✓ w0  List every .rs file under src/, recursively. Return o…   2s
   ✓ w1  Read crates/core/src/agent.rs and write ONE sentence …   3s
   ✓ w2  Read crates/core/src/repl.rs and write ONE sentence d…   4s
   …
   workflow done — 47 workers, total 1m 12s
   crates/core/src/agent.rs — the streaming agent loop
   crates/core/src/repl.rs — REPL command parser + rustyline I/O
   …
   ```

ถ้า worker error จะเห็น `✗ wN  …` และสคริปต์มักจะ catch แล้วทำงาน
ต่อ (แล้วแต่ model เขียน)

### เริ่มใช้ผ่าน model

งานเดียวกัน ไม่ต้องพิมพ์ slash command — บอกใน chat เลย:

```text
you > rewrite each test file under tests/ ให้ใช้ TestHarness fixture ใหม่
```

Model มอง pattern นี้เป็น fan-out แล้วตัดสินใจเรียก `WorkflowRun`:

```text
[approval] WorkflowRun(prompt: "rewrite each test file under tests/
                                 ให้ใช้ TestHarness fixture ใหม่")
[a]llow once · [A]lways · [d]eny: a
[workflow: author phase…]
[workflow: 32 subagent turn(s), 18432 in / 9621 out tokens]
✓ tests/test_login.rs — migrated to TestHarness::new()
✓ tests/test_signup.rs — migrated; 1 helper renamed
…
```

เห็น approval prompt 1 ครั้งสำหรับทั้ง call `WorkflowRun` (เหมือน
Bash) แต่ใน script — แต่ละ `Skill` / `Bash` / `Edit` call ยังต้อง
approval ของตัวเองตามปกติ — `WorkflowRun` ไม่ bypass per-tool gate

ถ้าอยากเห็น JS script ที่ model เขียน ใช้ slash-command แทน
(`/workflow run <prompt>` เดียวกัน) — เส้น tool ข้าม review loop
เพื่อความเร็ว

## API `thclaws.*`

สคริปต์ของคุณได้ global ตัวเดียว — `thclaws` — มี field ต่อไปนี้:

```js
thclaws.subagent({
  prompt: string,           // จำเป็น — งานของ worker
  budget?: {                // enforce แล้วทั้งคู่
    time?: number | string, //   "60s" / "2m" / "1m30s" / 60 (เป็นวินาที)
    tokens?: number,        //   เพดาน input + output ต่อ worker
  },
  schema?: object,          // JSON Schema worker ถูกขอ JSON ที่
                            //   ตรง schema เมื่อสำเร็จจะคืน parsed value
                            //   (ไม่ใช่ text)
  retry?: number | {        // retry เมื่อ hard error + schema fail
    max: number,
    backoff?: string,       //   "exponential" / "linear" / "500ms" / ฯลฯ
  },
  caps?: {                  // grant แบบชัดเจน — default DENY ของ KMS write
    kms?: { write?: string[] },
  },
  agent?: string,           // รันเป็น named agent (.thclaws/agents/<name>.md)
                            //   ใช้ tools/instructions ของ agent นั้น ดูบทที่ 15
}) → string | parsed_value
```

**Schema จาก agent def (ไม่ต้องใส่ `schema` รายครั้ง)** ถ้า `agent` ที่ระบุ
ประกาศ `output_schema` ไว้ใน frontmatter (บทที่ 15) และการเรียกนั้นไม่ได้ใส่
`schema` มา output ของ worker จะถูก validate กับ schema ของ agent def แล้วคืน
ค่าที่ parse แล้ว เขียนสัญญาไว้ที่ agent ครั้งเดียว ไม่ต้องเขียนซ้ำในทุก
workflow ที่เรียกมัน ส่วน `schema` ที่ใส่มารายครั้งยังชนะเสมอ

**`thclaws.parallel([spec, …])` — fan-out จริง** ส่ง **array** ของ spec object
หน้าตาเดียวกัน `{prompt, agent?, schema?, caps?, budget?, fallback?}` เข้าไป
worker จะรัน **พร้อมกัน** แล้วคืน array ของผลลัพธ์ตามลำดับ input นี่คือ
primitive ตัวเดียวที่ขนานจริง — `Promise.all` ครอบ `thclaws.subagent` รันเรียง

**เพดานการรันพร้อมกันคือ 8** เปลี่ยนได้ด้วย environment variable
`THCLAWS_WORKFLOW_PARALLELISM` (บีบอยู่ในช่วง 1–32)

```sh
THCLAWS_WORKFLOW_PARALLELISM=16 thclaws --workflow ./render-all.js
```

เลข 8 เป็นตัวเลขเชิง *I/O* ไม่ใช่เชิง CPU เพราะทุก future ที่รันขนานเอาแต่รอ
model call อยู่ ตัวจำกัดจริงคือ gateway หรือ provider ของคุณรับ concurrent ได้
แค่ไหน ไม่ใช่จำนวน core ถ้า provider รับไหวก็เพิ่มได้ ถ้าโดน rate-limit ก็ลด

> build เก่าเคยคำนวณเพดานจาก `min(16, cores-2)` ซึ่งเป็นมาตรวัดของงาน CPU-bound
> ที่เอามาใช้กับงาน I/O-bound และบน cloud runner ที่มี core เดียวมันยุบเหลือ 1
> — ทำให้ทุก stage ของ `thclaws.parallel` ถูกรันเรียงเงียบ ๆ ทั้งที่ดูเหมือน
> รันขนานอยู่ ถ้าคุณมีโน้ตหรือสคริปต์ที่อิงสูตรจำนวน core นั่นคือพฤติกรรมเก่า

**มัน settle ไม่ reject** worker ที่ล้มเหลวหลัง retry หมดแล้วจะ **ไม่** ล้ม
ทั้ง batch ช่องนั้นจะกลายเป็นค่า **`fallback`** ของ spec (ค่าเริ่มต้น `null`)
งาน 50 ชิ้นที่ worker ตายไปหนึ่งตัวจึงเหลืออีก 49 ชิ้นครบ ควรใส่ `fallback`
เป็น record ที่พก id ที่ปลายทางต้องใช้ไว้ด้วย ความล้มเหลวจะได้ระบุตัวได้ ไม่ใช่
`null` เปล่า ๆ การเรียกจะ throw เฉพาะกรณีที่เป็นความผิดของโปรแกรมเมอร์เท่านั้น
(อาร์กิวเมนต์ไม่ใช่ array, surface นี้ไม่มี tool `Task`)

`caps` ของแต่ละ worker ถูกแยกราย future การ grant สิทธิ์เขียน KMS จึงไม่รั่ว
ข้าม batch หมายเหตุ: soft-cap ของ token budget รายตัวและ replay cache สำหรับ
resume ที่ `thclaws.subagent` มีให้ **ไม่ถูกใช้** บนเส้นทาง parallel (แต่ยัง
นับ usage รวมตามปกติ)

```js
const images = thclaws.parallel(
  subjects.map((s) => ({
    agent: "image-smith",
    prompt: `render ${s.name}`,
    budget: { time: "5m" },
    fallback: { slug: s.slug, status: "failed" }, // ค่าของช่องนี้เมื่อ worker ล้ม
  }))
);
```

**`thclaws.pollUntil(checkFn, opts)` — submit→poll→done** เรียก `checkFn()`
ทุก ๆ `opts.interval` จนกว่า `opts.until(result)` จะเป็นจริง (หรือตัว result
เองเป็นค่าจริง) แล้วคืนค่านั้น ถ้าเกิน `opts.timeout` จะ throw มีขอบเขตและรับรู้
การ cancel — เป็นวิธีที่ถูกต้องสำหรับรองาน async (image/video/TTS) แทนการเขียน
loop เอง `{ interval: "10s", timeout: "10m", until: r => r.state === "done" }`

สคริปต์ยังได้ global เพิ่มอีกสามตัว

- **`thclaws.log(msg)`** — พ่นบรรทัดบรรยายเพื่อสังเกตการณ์ (sandbox ถอด
  `console` ออก ตัวนี้จึงเป็นช่องทางที่ถูกต้องสำหรับ trace งานหลาย stage)
  ไม่คืนค่าอะไร
- **`thclaws.include(path)`** — ดึงไฟล์ `.js` อีกตัว (helper, prompt string
  ที่ใช้ร่วม) แบบ relative กับ directory ของสคริปต์ การ traverse (`..`),
  absolute path และ symlink ที่หลุดออกนอก base dir ถูกปฏิเสธ
- **`args`** — input แบบมีโครงสร้างที่ส่งผ่าน
  `WorkflowRun({ script_path, args })` (เป็น JSON ค่าอะไรก็ได้ ถ้าไม่ส่งมาจะ
  เป็น `null`) อ่านตรง ๆ ได้เลย เช่น `const q = args.query;`

**`caps.kms.write` ควบคุมว่า worker เขียน KMS อะไรได้** นอก workflow
KMS write tool ทำงานปกติ ใน `/workflow run` worker default = **deny**
ทั้งหมด ต้องผ่าน `caps: { kms: { write: ["scratch", "audit-log"] } }`
จึงจะ grant เป็นราย call grant ถูกบันทึก `worker_caps` ใน
state.jsonl และ **ไม่ transitive** — Task spawn ของ worker เองจะได้
caps เปล่าใหม่ ถ้าไม่ grant ใหม่อีกครั้ง

Time budget ครอบ worker call ด้วย `tokio::time::timeout` พอเกินจะ
throw Schema validation รันหลังทุก attempt — ถ้า worker output ไม่
parse เป็น JSON หรือไม่ตรง schema จะ retry ตาม `retry.max` ด้วย
`backoff` ที่เลือก ทุก retry บันทึก `worker_retry` event ให้
`/workflow inspect <id>` เห็น chain Worker จะ inherit provider, model, system prompt,
tool registry, memory, KMS, และ permission mode จาก session แม่ —
ดังนั้น worker ใช้ `Bash`, `Read`, `Edit`, search KMS, MCP server
ได้หมด การ recurse ของ subagent (worker เรียก Task เอง) ถูกจำกัด
ด้วย `DEFAULT_MAX_DEPTH = 3` เหมือนกับ subagent ปกติ

**Async syntax ใช้ได้แล้ว** — script ที่ใช้ `await` / `async` /
`Promise.all` จะถูก route ผ่าน Boa Module mode `thclaws.subagent`
ยัง synchronous ภายใน ดังนั้น `Promise.all([...])` resolve ได้แต่
worker รันตามลำดับใน source (ทีละตัว) ถ้าต้องการขนานจริง ให้ส่ง spec
เข้า `thclaws.parallel([...])` แทน (ดูข้างบน)

### เขียนอะไรในสคริปต์ได้บ้าง

JS control flow: `for`, `while`, `if`/`else`, `try`/`catch`, `await`,
`async` function, `Promise.all`, destructuring, template literal,
array/string method, regex, JSON parsing

### เขียนอะไรไม่ได้

- `eval`, `Function` (ถูกปลดจาก sandbox)
- `fetch`, `require`, `process`, DOM, `console.log`

ของที่จะ I/O ต้องผ่าน subagent

### ตัวอย่างสั้น ๆ

```js
// Workflow: list .rs files, summarise each
const list = await thclaws.subagent({
  prompt: "List every .rs file under src/, recursively. Paths only."
});
const paths = list.split("\n").map(s => s.trim()).filter(Boolean);

const summaries = await Promise.all(
  paths.map(p => thclaws.subagent({
    prompt: `Read ${p} and write ONE sentence describing what it does.`
  }))
);

paths.map((p, i) => `${p} — ${summaries[i]}`).join("\n");
```

สำหรับ script แบบ sync **expression สุดท้าย** คือผลลัพธ์ ส่วน script
แบบ async (Module mode) ใช้ expression สุดท้ายที่ auto-wrapper หา
เจอ หรือใส่ `globalThis.__wf_result = …` เองก็ได้ ถ้าไม่มีทั้งสอง
อย่างจะคืน `undefined`

## State บนดิสก์

ทุกครั้งที่รัน workflow จะเขียน JSONL log ลง:

```text
.thclaws/state/workflows/wf-<id>/state.jsonl
```

หนึ่ง event ต่อบรรทัด flush หลังเขียนทุกครั้งเพื่อให้ Ctrl-C ไม่
ทิ้งไฟล์ค้างกลางคัน รูปแบบ event:

```jsonl
{"ts":"…","kind":"start","id":"wf-…","prompt":"…","script_sha":"…","script_chars":234}
{"ts":"…","kind":"worker_start","id":"wf-…","worker":"w0","prompt":"…"}
{"ts":"…","kind":"worker_done","id":"wf-…","worker":"w0","output":"…"}
{"ts":"…","kind":"worker_error","id":"wf-…","worker":"w1","error":"…"}
{"ts":"…","kind":"done","id":"wf-…","result":"…"}
```

`cat`, `grep`, `jq` ไฟล์ได้ตลอดเวลา — เป็น JSONL ธรรมดา ไม่มี
ฟอร์แมตปิด ไฟล์ `script.js` (JS ที่ approve แล้ว) ก็ถูกเขียนไว้
ข้างกัน เผื่อ `/workflow resume <id>` จะ replay จาก source เดิม

Slash command สำหรับจัดการ run (ใช้ได้ทั้ง CLI REPL และ GUI chat tab
**ยกเว้น** `/workflow rm` ที่ต้อง y/N confirm แบบ interactive ซึ่ง
chat tab แสดงไม่ได้ — ต้องรันจาก `thclaws --cli`):

```text
/workflow list             หนึ่งบรรทัดต่อ run จากใหม่ไปเก่า
/workflow inspect <id>     dump state.jsonl events
/workflow resume <id>      รันใหม่ replay worker ที่เสร็จจาก cache
                           ส่วน fresh spawn เลขต่อไปเรื่อย ๆ
/workflow rm <id>          ถาม y/N แล้วลบทั้ง directory
```

ชื่อย่อ เผื่อจำคำเต็มไม่ได้: `list` ใช้ `ls` ได้ `inspect` ใช้ `show`
หรือ `cat` ได้ `rm` ใช้ `delete` หรือ `del` ได้ `exec` ใช้ `file` หรือ
`script` ได้ ส่วน `/wf` เป็นตัวย่อของ `/workflow`

`resume` match ด้วย **prompt** ที่ทุก `thclaws.subagent` call cache
entry จะถูกใช้ก็ต่อเมื่อ prompt ตรง mismatch จะ fall-through ไป
spawn ใหม่ (script อาจถูกแก้หรือ path เปลี่ยน) ถ้ามี cache เหลือ
ตอนจบ script จะรายงาน "diverged" ให้รู้

ถ้า `.thclaws/` เขียนไม่ได้ (read-only volume, permission)
workflow ยังรันแต่จะ print:
```text
/workflow run: state.jsonl unavailable — proceeding without checkpoint
```
audit trail หายไป แต่ run ไม่หาย

## รัน script ที่เขียนไว้ล่วงหน้า (ข้าม author phase)

ถ้าคุณมีไฟล์ workflow `.js` บนดิสก์อยู่แล้ว — ที่เขียนเอง หรือ
`script.js` ที่เซฟไว้จาก `/workflow run` ครั้งก่อน — รันตรง ๆ ได้เลย
โดยข้าม author + review phase ทั้งหมด:

- **Mid-session:** `/workflow exec <path>` (alias `file` / `script`)
  รัน script จากดิสก์ใน REPL **หรือ** GUI session ปัจจุบัน
- **Headless:** `thclaws --workflow <file.js>` รันจาก command line —
  เหมาะกับ CI, cron job (บทที่ 19), deploy hook

ทุก `/workflow run` จะ persist script ที่ approve แล้วไว้ที่
`.thclaws/state/workflows/wf-<id>/script.js` ดังนั้น workflow ที่ model
author ครั้งเดียวเอามารันซ้ำแบบ deterministic ได้ด้วย `/workflow exec`
ชี้ path นั้น (หรือ replay จาก checkpoint ด้วย `/workflow resume <id>`)

```sh
# รันใหม่:
thclaws --workflow ./scripts/audit-crates.js

# Resume จาก id เดิม (หรือ prefix):
thclaws --workflow ./scripts/audit-crates.js --resume wf-18b3fa

# stdout = ค่าสุดท้ายของ script; stderr = id + done summary
# pipe stdout เข้า jq, redirect ลงไฟล์ ฯลฯ ได้:
thclaws --workflow ./scripts/audit-crates.js > result.txt
```

Exit code = 0 เมื่อสำเร็จ, 1 เมื่อ script fail **ทั้ง** `/workflow
exec` และ `--workflow` จะ auto-approve tool call ทุกตัวที่ worker เรียก
(เหมือน `--dangerously-skip-permissions`) — ถือว่า script ผ่านการ vet
จาก operator แล้ว ดังนั้นรันเฉพาะ script ที่ไว้ใจได้

> `thclaws -p "/workflow run <goal>"` ไม่ใช่เส้นที่ใช้งานได้จริง:
> print mode แบบ one-shot ไม่มี surface สำหรับ author/review loop และ
> host function `thclaws.subagent` ก็ไม่ถูก wire ในนั้น สำหรับการรัน
> แบบ non-interactive ให้ pre-author script แล้วใช้ `--workflow`

## ข้อจำกัดปัจจุบัน

นี่คือช่องว่างที่รู้อยู่ ไม่ใช่ bug — track ไว้ใน
[dev-plan/32](../dev-plan/32-dynamic-workflows.md) (workspace-only):

- **`Promise.all` ครอบ `thclaws.subagent` ยังไม่ขนานจริง** host
  function ตัวนั้น block JS thread ต่อหนึ่งการเรียก subagent call ที่
  อยู่ใน `Promise.all` จึงยังรันเรียงกัน (wall clock = ผลรวม ไม่ใช่ค่า
  มากที่สุด) ให้ใช้ **`thclaws.parallel([...])`** สำหรับการขนานจริง —
  มันรัน worker บน tokio runtime ครั้งละ 8 ตัวโดยค่าเริ่มต้น ช่องว่าง
  ที่เหลืออยู่คือทำให้ `Promise.all` เองซ้อนทับกันได้ จนกว่าจะถึงตอนนั้น
  `thclaws.parallel` คือการ opt-in แบบเขียนชัด
- **ยังไม่มี verification phase** primitive `thclaws.verify({...})`
  เฉพาะยังไม่มี — script ที่อยากมีขั้น verify ให้เขียนเป็น
  `thclaws.subagent` อีก call
- **ยังไม่มี GUI worker grid แบบ real-time** `/workflow run` และ
  คำสั่งจัดการรันได้จาก chat tab แต่ progress ของ worker stream เป็น
  text ไม่ใช่ grid สด UI progress แบบ real-time ยังเป็นงาน frontend

## เรื่อง cost

ทุก `thclaws.subagent` call เป็น model turn แยก — ปกติไม่กี่วินาที
และไม่กี่ร้อยถึงไม่กี่พัน token Workflow 200 worker อาจกิน $5–$20
ของ API token ได้ง่าย ๆ ขึ้นกับ model มี 2 guard ใช้งานจริง:

- **จำกัด fan-out ก่อนเขียนสคริปต์** ถ้าเป้าหมายไม่มีขอบเขต ("ทุก
  ไฟล์") ให้ discovery subagent คืน list ก่อนจะได้เห็น cardinality
  ก่อน approve สคริปต์
- **ดูบรรทัดสรุปปิดท้าย** `workflow done — N workers, total Xs, In
  tokens / Out tokens (≈$Y.YY)` พิมพ์ทุกครั้งหลัง `/workflow run`
  โมเดลที่เป็น tier-billed หรือไม่รู้ราคาจะแสดง `(cost unknown)`
  แทนตัวเลข

## ตารางอ้างอิงเร็ว

| | Subagent (`Task`) | `/agent` | Agent Teams | Workflow |
|---|---|---|---|---|
| ตัวสั่งการ | Model | คุณ (one-shot) | Team-lead model | Code |
| จำนวน worker | 1 (blocking) | 1 (concurrent) | 3–5 collaborator | สิบถึงร้อย |
| worker คุยกันเอง | ไม่ได้ | ไม่ได้ | ได้ (mailbox) | ไม่ได้ (stateless) |
| Determinism | Model-driven | Model-driven | Model-driven | Deterministic execution |
| Resume ได้ | ไม่ | ไม่ | จำกัด | ได้ (`/workflow resume` replay จาก checkpoint) |
| เหมาะกับ | Side-quest กลางเทิร์น | Specialist ทำงานคู่ขนาน | ถกเถียง / ร่วมมือ | Bulk fan-out |

## Troubleshooting

**"workflow: state.jsonl unavailable — proceeding without checkpoint"**
— `.thclaws/state/workflows/` สร้างหรือเขียนไม่ได้ ตรวจ permission ของ
`.thclaws/` ใน project root

**Script error: `ReferenceError: thclaws is not defined`** — คุณ
น่าจะรันสคริปต์นอก `/workflow run` global `thclaws.*` มีอยู่เฉพาะ
ใน workflow sandbox

**Workflow ค้างหลังบรรทัด `⠋ wN  …`** — worker ตัวนั้นกำลังใช้เวลา
นาน ถ้าไม่ได้ตั้ง `budget: { time }` subagent call จะรันไม่จำกัดเวลา
กด Ctrl-C จะหยุดทั้ง run ใส่ `time` budget ต่อ worker ในสคริปต์เพื่อ
จำกัดเวลาราย worker ได้

**Re-author loop ได้สคริปต์เดิมซ้ำ ๆ** — model อาจมอง revision
note ของคุณข้าม ลองยกเลิกแล้วรันใหม่โดยเขียน goal ให้ชัดขึ้น แทน
การพึ่ง `r`-loop
