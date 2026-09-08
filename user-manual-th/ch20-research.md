# บทที่ 20 — Research เบื้องหลัง (`/research`)

`/research <query>` รันงานเบื้องหลังที่อ่านเว็บแล้วเติบโต knowledge base ของคุณแบบ **zettelkasten**: หนึ่ง note ต่อหนึ่งความคิด ทุก claim ยึดกับ quote คำต่อคำจากแหล่งที่มา note เชื่อมโยงถึงกันและถึงสิ่งที่ KMS รู้อยู่แล้ว query ที่สองในหัวข้อใกล้เคียงจะ **อัปเดต** note เดิม ไม่ใช่เขียน page ชุดใหม่ขนานกัน

## เริ่มต้นอย่างเร็ว

```
> /research กฎหมายแรงงานไทย ค่าจ้าง ค่าล่วงเวลา และการเลิกจ้าง
[research started: id=research-523f9c5b] query: กฎหมายแรงงานไทย …
  /research status research-523f9c5b     ดูความคืบหน้า
  /research show research-523f9c5b       เปิด map of content
  /research cancel research-523f9c5b     ยกเลิก
…
[research done: id=research-523f9c5b → rv2-labour/rv2-labour.md]
```

run ทั่วไป: 3 รอบของการค้นหา + อ่าน, 25–30 แหล่ง, 10–12 note, 3–7 นาทีบน worker model ที่เร็ว GUI แสดงแผง "Research" สดๆ ที่ขอบขวา (phase, รอบ, novelty) ส่วน CLI พิมพ์บรรทัดแจ้งเสร็จเหนือ prompt ถัดไป

## สิ่งที่ได้

```
<kms>/
├── pages/
│   ├── overtime-pay.md              ← หนึ่งความคิดต่อ note (kind: concept)
│   ├── severance-pay.md
│   ├── labour-protection-act-be-2541.md   (kind: entity)
│   ├── thai-labour-law.md           ← map of content ของ query (kind: moc)
│   └── …
├── sources/<url-slug>.md            ← สำเนาของทุกแหล่งที่ถูกอ้าง
├── runs/2026-09-06-thai-labour-law.md   ← run ทำอะไรบ้าง (รอบ, novelty, note)
└── .research/                       ← cache ของ digest + ทะเบียน citation (ภายใน)
```

### Note

ทุก note คือ entity, concept หรือ claim เดียว มี slug คงที่ที่เป็นชื่อของความคิดนั้นเอง (`overtime-pay`, `wage-committee`) บรรทัดแรกเป็น abstract หนึ่งประโยค (กลายเป็น summary ใน index) ตามด้วย section สั้นๆ 2–4 หัวข้อ และ link `[[slug]]` ไปยัง note ที่เกี่ยวข้อง ทั้งจาก run นี้และที่มีอยู่แล้วใน KMS frontmatter:

```yaml
type: note
kind: concept          # entity | concept | claim | moc
title: ค่าล่วงเวลา
related: ["employee", "employer", "holiday", "working-day"]
sources: [1, 4, 18, 27]
claims: 8
confidence: 0.93       # ค่าเฉลี่ย confidence ของ claim ที่ใช้
```

ข้อเท็จจริงมี citation `[N]` ที่ลิงก์ไปสำเนาแหล่ง และ block `## Sources` ที่สร้างให้อัตโนมัติ ไม่มี appendix "research notes" ไม่มี section verification: ไม่มีอะไรลงใน note เว้นแต่มาจาก claim ที่ผ่าน quote check

### หน้า topic (map of content)

หนึ่ง note ต่อ run, `kind: moc`, slug มาจาก query หน้านี้คือผลลัพธ์ของ research: รายงานที่ **อธิบายหัวข้อ** — เปิดด้วยคำตอบตรง ๆ ตามด้วย 4–8 หัวข้อที่สังเคราะห์จาก claim ทั้งหมด (ภาพรวม, ผู้เล่นหลัก, ตารางเปรียบเทียบเมื่อหลาย entity มีคุณสมบัติเทียบกันได้, timeline เมื่อ claim มีวันที่, เงินทุน/นโยบาย, แนวโน้ม) และลิงก์ note ลูกในเนื้อหาตรงจุดที่เล่าถึงครั้งแรก ปิดท้ายด้วย `## Map` ของทุก note ที่ลิงก์ และ open questions ไม่เกิน 3 ข้อ `/research show <id>` เปิดหน้านี้

การวางแผนทำแบบ top-down: worker model ตัดสินใจก่อนว่าหน้า topic ควรอธิบายเรื่องนี้อย่างไร แล้วจึงเลือกว่าหน้านั้นต้องลิงก์ไปอะไรบ้างเพื่อลงลึก จากนั้นค่อยจ่าย claim note ลูกถูกเขียนหลังหน้า topic โดยเห็นย่อหน้าเปิดของหน้าแม่ จึงลงลึกแทนที่จะเล่าซ้ำ

### Claim และ quote check

แต่ละแหล่งถูกอ่าน **ครั้งเดียว** แล้วกลายเป็น digest: entity ที่พูดถึงและ claim ไม่เกิน 6 ข้อ แต่ละข้อมี **quote คำต่อคำ** (≤ 60 ตัวอักษร) จากหน้านั้น pipeline ตรวจว่า quote อยู่ในข้อความที่ fetch มาจริง (ไม่สนช่องว่างและตัวพิมพ์ normalize Unicode แล้ว) และทิ้ง claim ที่ไม่ผ่าน ในทางปฏิบัติ 15–20% ของสิ่งที่โมเดลเสนอถูกทิ้งด้วยวิธีนี้ ซึ่งคือ citation ปลอมที่ verifier แบบเดิมต้องไล่ล่าทีหลัง แต่ตอนนี้ถูกจับก่อนถูกเขียน digest ถูก cache ตาม URL ใต้ `.research/digests/` การ run ซ้ำบนแหล่งเดิมจึงไม่มีค่าใช้จ่าย

### run อ่านแหล่งไหน

search engine จัดอันดับตาม engagement คำค้นเรื่องโมเดลใหม่ของผู้ผลิตรายหนึ่งจึงคืนทั้ง release note ของผู้ผลิตเอง *และ* บทความสรุปอีกห้าชิ้นที่เล่าซ้ำ ก่อน fetch ทุกรอบจะจัดอันดับผู้สมัครก่อน:

1. **Primary มาก่อน** — โดเมนที่สะกดชื่อเรื่องนั้น (`deepseek.com` สำหรับ DeepSeek), เอกสารทางการ และ `.gov` / `.edu` / `.ac.*` / `.go.*`
2. **Reference ถัดมา** — Wikipedia, arXiv และสำนักข่าวใหญ่
3. **ที่เหลือ** ตามลำดับของ search engine

ไม่มีการตัดแหล่งใดทิ้ง การจัดอันดับแค่ตัดสินว่าใครถูกอ่านก่อนภายในโควตาของรอบนั้น และ **หนึ่งโดเมนให้ได้ไม่เกินสองหน้าต่อรอบ** เว็บเดียวจึงเป็นหลักฐานครึ่งหนึ่งของ run ไม่ได้ ตาราง **Evidence** ใน run log บอกว่ารอบนั้นอ่านแหล่งระดับไหนไปกี่แห่ง และ note อ้างอิงจริงกี่แห่ง

### Citation คงที่ต่อ KMS

URL หนึ่งได้เลข citation หนึ่งตลอดอายุของ knowledge base (`.research/sources.json`) เมื่อ run ทีหลัง merge claim ใหม่เข้า note เดิม เครื่องหมาย `[N]` ที่เขียนไว้เดือนก่อนยังชี้ถูก

หน้าเดียวกันที่มาด้วย URL ต่างรูปคือแหล่งเดียวกัน: tracking parameter (`?utm_source=…`), `#fragment`, `www.`, `http` เทียบ `https` และ `/` ปิดท้าย ถูกตัดก่อนค้นหา run จึงไม่จ่ายซ้ำและไม่อ้างเอกสารเดียวด้วยสองเลข

## รอบและการหยุดด้วย novelty

1. **รอบ seed** — ค้นหา query อ่าน 5 หน้าแรก digest
2. **รอบเติมช่องว่าง** — จากตาราง entity และ claim (ไม่ใช่หน้าดิบ) worker model เสนอการค้นหาใหม่ไม่เกิน 4 รายการ: entity ที่ยังบาง, คำถามย่อยที่ยังไม่มี claim, มุมมองตรงข้าม, แหล่งปฐมภูมิ URL ใหม่ถูก fetch และ digest แบบขนาน
3. **หยุด** เมื่อ *entity ใหม่* ของรอบต่ำกว่าเกณฑ์ novelty (ค่าเริ่มต้น 35%) เมื่อไม่มีการค้นหาที่มีประโยชน์เหลือ หรือถึง `--max-iter` (ค่าเริ่มต้น 4) ไม่มีการให้คะแนนตัวเองโดย LLM
4. **วางแผน** — หนึ่ง call เปลี่ยนตารางบวกรายการ note ที่มีอยู่เป็นแผน โดยเริ่มจากหน้า topic: โครงหัวข้อและ claim ที่เนื้อเรื่องต้องใช้ แล้วจึงเป็น note ที่หน้านั้นลิงก์ไป (`create` หรือ `update` ต่อ slug, entity ที่ note นั้นครอบคลุม, เหตุผลที่หน้า topic ลิงก์มา) claim ถูกผูกเข้า note ตาม entity tag โดยอัตโนมัติ — แผนไม่ต้องระบุ claim id จึงเล็กพอที่จะไม่ถูกตัด — และหน้า topic ได้รับทุก claim
5. **เขียน** — หน้า topic ก่อน แล้วหนึ่ง call ต่อ note ลูกแบบขนาน จาก claim ของ note นั้นบวกย่อหน้าเปิดของหน้า topic การอัปเดต merge เข้า body เดิม (หรือเพิ่ม section `## Update` ที่มีวันที่ด้วย `--append`)

## ความเป็นปัจจุบัน

ทุก prompt ใน pipeline ถูกยึดกับวันที่ปัจจุบันและถูกบอกว่าความจำของโมเดลเรื่อง "เวอร์ชันล่าสุด" เก่าแล้ว ให้แหล่งข้อมูลเท่านั้นเป็นผู้ตัดสิน รอบ seed ค้นหาเพิ่มอีกหนึ่งรายการแบบกรองความสด (`<query> latest <ปี>` ผลลัพธ์ในรอบปีบน Tavily/Brave) ทุกรอบเติมช่องว่างต้องมี query "newest release <ปี>" อย่างน้อยหนึ่งรายการ และ query ที่ระบุปีปัจจุบันหรือคำว่า latest/newest จะผ่านตัวกรองความสดด้วย digest บันทึกวันที่เผยแพร่ของแต่ละหน้า เมื่อ claim ขัดกัน วันที่ใหม่กว่าชนะ และสิ่งที่เปลี่ยนตามเวลาจะถูกเขียนว่า "as of <วันที่>" ก่อนหน้านี้ run ในปี 2026 เรื่อง "Chinese AI companies" ปักการค้นหาไว้ที่ 2025 และรายงาน DeepSeek V3 กับ Qwen 3.6 เป็นรุ่นล่าสุด

### run ไม่ทิ้งสิ่งที่อ่านมาแล้ว

body ของแต่ละ note คือ call แยกกัน ถ้าตัวหนึ่งล้มเหลว (timeout, คำตอบถูกตัด) จะเสียแค่ note นั้น note อื่น ๆ, source และ run log ยังถูกเขียนครบ และส่วน **Warnings** ใน run log จะบอกว่าข้ามอะไรไป run จะล้มเหลวทั้งหมดก็ต่อเมื่อเขียน note ไม่ได้เลยสักหน้า

run สองครั้งบนหัวข้อเดียวกันในวันเดียวกันได้ run log สองไฟล์ (`…-topic.md`, `…-topic-2.md`) บันทึกของครั้งแรกจึงไม่หาย

## ทำให้ note เป็นปัจจุบัน: `/research refresh`

```
/research refresh <slug>                          refresh note เดียวใน KMS ที่ attach อยู่
/research refresh <kms> <slug> [<slug>…]          …ใน KMS ที่ระบุ
/research refresh [<kms>] --all [--older-than 30]  ทุก note (ไม่รวม MOC) ที่ไม่ได้อัปเดตใน N วัน
```

refresh ใช้ชื่อ note เป็น query ค้นหาแบบสั้นสองรอบพร้อมตัวกรองความสด แล้วบังคับให้แผนเป็น `update` note นั้น: claim ใหม่ถูก merge เข้าไป ข้อเท็จจริงที่ถูกแทนที่ถูกเขียนใหม่ว่าเป็นของเก่า และวันที่ `updated` เลื่อนไป ไม่มีการเขียน map of content ใหม่ job เข้าคิวและรันทีละอัน แผง Research ตามแต่ละ job context menu ของ page ใน KMS sidebar มีคำสั่งเดียวกัน ("Refresh (research)")

## Subcommand

```
/research <query>                            เริ่ม (pipeline v2)
/research [flags…] <query>                   เริ่มพร้อม override
/research                                    list ทุก job (ใหม่สุดก่อน)
/research status <id>                        phase, รอบ, แหล่ง, novelty
/research show <id>                          พิมพ์ map of content ในแชท
/research cancel <id>                        ยกเลิก ไม่มีอะไรเขียนครึ่งๆ กลางๆ
/research wait <id>                          block prompt ของ CLI จนเสร็จ
```

### Flag

| Flag | ค่าเริ่มต้น | ทำอะไร |
|---|---|---|
| `--kms <name>` | KMS ที่ attach ล่าสุด ไม่งั้นสร้างใหม่จากชื่อ query | KMS เป้าหมาย `--kms new` บังคับสร้าง KMS ใหม่ KMS ที่ run เขียนลงจะถูก attach กับโปรเจกต์อัตโนมัติ query ถัดๆ ไปจึงต่อ graph เดิม |
| `--lang th\|en\|…` | `th` | ภาษาของ body ของ note, ข้อความ claim และชื่อเรื่อง ศัพท์เทคนิค ชื่อโมเดล/API และโค้ดคงเป็นภาษาอังกฤษทุกภาษา `--lang query` ใช้ภาษาเดียวกับ query |
| `--max-notes N` | 30 (สูงสุด 50) | เพดาน note ต่อ run รวมหน้า topic entity ที่บางจะถูกรวมเข้ากับหน้าแม่แทนการตัดทิ้ง |
| `--min-iter N` / `--max-iter N` | 2 / 4 | พื้นและเพดานของรอบ |
| `--novelty 0.X` | 0.35 | หยุดเมื่อรอบเพิ่ม entity ใหม่น้อยกว่าสัดส่วนนี้ `1.0` = วิ่งถึง `--max-iter` เสมอ |
| `--worker-model <id>` | โมเดลปัจจุบันของคุณ | โมเดลสำหรับ digest, gap query, แผน และ body ของ note thClaws ไม่สลับโมเดลแทนคุณ ระบุที่นี่ถ้าอยากให้ research รันบนโมเดลที่เร็วกว่าโมเดลแชท |
| `--append` | ปิด | ไม่เขียนทับ note เดิม เพิ่ม section `## Update` ที่มีวันที่แทน สำหรับ KMS ที่ต้องมี audit trail |
| `--dry-run` | ปิด | อ่าน digest และวางแผน แล้วเขียนแค่ run log ไม่แตะ KMS อย่างอื่น |
| `--budget-time 20m` | 25m | เพดานเวลา เกินแล้ว job จบเป็น failed |
| `--legacy` | ปิด | pipeline แบบ page ก่อน v0.121 (`--max-pages`, `--score-threshold`, verify pass) จะถูกลบใน release ถัดไป |

`--max-pages` รับเป็นชื่อแทนของ `--max-notes`

## ทำไม worker model จึงสำคัญ

การ digest และเขียน note เป็นงานเชิงกลและรันขนาน เวลาที่ใช้จริงคือ latency ของ call เดียว ไม่ใช่จำนวน call วัดบน workspace นี้: Gemini 2.5 Flash ตอบ call ภาษาไทย 4 อันพร้อมกันใน 5–7 วินาทีต่ออัน; DeepSeek v4 Flash เข้าคิวห่างกันราว 50 วินาที; reasoning model ใช้เป็นนาที บน flash model ที่เร็ว รอบหนึ่งใช้ 30–40 วินาที บนโมเดลที่ตอบทีละ call หรือ reasoning model run เดียวกันใช้ 15–25 นาที และ engine จะ log คำแนะนำหลังรอบแรก pipeline หยุดเพิ่มรอบค้นหาเมื่อใช้ time budget ไป 60% แล้วเขียนด้วยสิ่งที่มี โมเดลช้าจึงได้ผลลัพธ์เล็กลง ไม่ใช่ล้มเหลว

## ความคืบหน้าสด

**GUI** — แผง Research ขอบขวาแสดง phase (`round 2/3: reading 8 sources`, `planning notes`, `writing 11 notes`), แถบรอบ และประวัติต่อรอบที่แถบคือ novelty ของรอบนั้น

**CLI** — บรรทัดแจ้งเสร็จเหนือ prompt ถัดไป:

```
[research done: id=research-523f9c5b → rv2-labour/rv2-labour.md]
[research failed: id=research-x9z8] research time budget exhausted
```

stderr ของ engine ยัง log แต่ละรอบ (`[research] round 2: 15 sources, 64 new / 79 (71% novelty), 35.7s`), แต่ละ digest, แผน และ wave การเขียน note ซึ่งเป็นที่แรกที่ควรดูเมื่อ run ช้า

## เคล็ดลับ

- query ต่อเนื่องลง KMS เดียวกันโดยค่าเริ่มต้น (อันที่ attach ล่าสุด) ใช้ `--kms new` เมื่ออยากแยก graph จริงๆ และ `/kms use <name>` เพื่อเปลี่ยนว่า KMS ไหนเป็นค่าเริ่มต้น
- เริ่มด้วย `--dry-run` บนหัวข้อใหม่เพื่อดูแผนก่อนจ่ายค่า note
- ใช้ `--append` บน KMS ที่คนอื่นอ่าน เพราะการ merge เงียบ
- เปิด run log ใต้ `runs/` เพื่อดูว่าค้นหาอะไรไปบ้างและ quote check ทิ้ง claim กี่ข้อ

## แก้ปัญหา

| อาการ | สาเหตุ | แก้ |
|---|---|---|
| `research time budget exhausted` | โมเดลช้า หรือเว็บช้าหลายแห่ง | ดูเวลาใน stderr; ตั้ง `--worker-model`; `--budget-time 40m` |
| digest ใช้เวลา 60–90 วินาทีต่อครั้งบน DeepSeek/Qwen | reasoning token บน prompt สกัดข้อมูลยาว | ไม่ต้องทำอะไร — research รัน worker call ที่ระดับ thinking `off` อยู่แล้ว (ดูบทที่ 6) ค่า `/thinking` ของแชทไม่ถูกแตะ |
| `research found no verifiable claims` | ทุกแหล่งเป็นหน้า navigation/listing หรือ digest model ไม่คืนอะไร | ลอง query ที่เจาะจงกว่า; `--worker-model` |
| note มี `[c:…]` ค้างในข้อความ | writer ใส่ claim id ที่ไม่อยู่ในแผน | ไม่เป็นอันตราย แก้ออกได้ build ปัจจุบันแก้แล้ว |
| MOC ลิงก์ไป note ที่ไม่มี | แผนอ้าง slug ที่ถูกตัดเพราะไม่มี claim | build ปัจจุบันเปลี่ยน link ไป slug ที่ไม่รู้จักเป็นข้อความธรรมดา |
| `/research show` เปิด run log | run นั้นเป็น `--dry-run` | รันใหม่โดยไม่ใส่ |
