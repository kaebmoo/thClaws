# บทที่ 21 — LINE chat & web browser bridge

ขับ thClaws จากมือถือ LINE OA `@thClaws` ทำหน้าที่ระบุตัวคุณและส่ง
link ให้ ส่วนตัวแชทเกิดในเว็บเบราว์เซอร์ตัวไหนก็ได้ โดยขับ Rust
agent loop เดียวกันบน desktop ของคุณ เปลี่ยนแค่ surface เท่านั้น
เพิ่มมาตั้งแต่ v0.9.0+ ผ่านชุด plan-07 / plan-08 / plan-10

## ทำไมต้องใช้

- approve คำสั่ง `Bash` จากมือถือ ขณะที่ desktop รันอัตโนมัติ
  อยู่บ้าน
- คุยกับ agent ต่อจากที่ไหนก็ได้ — พิมพ์ในเบราว์เซอร์บนมือถือ
  ส่วน tool registry เต็มชุดบน desktop (Bash, Edit, KMS, MCP,
  skills) ยังรันในเครื่อง
- สั่งงานยาว ๆ โดยไม่ต้องนั่งติดโต๊ะ

desktop ไม่หายไปไหน — code, secret, tool ทั้งหมดยังอยู่ในเครื่อง
มือถือ/เบราว์เซอร์เป็นแค่ bridge สำหรับอ่าน+พิมพ์เท่านั้น

## ทำงานยังไง (พารากราฟเดียว)

Axum service เล็ก ๆ ที่ `line.thclaws.ai` (และ `chat.thclaws.ai`
สำหรับ browser variant) ถือ WebSocket จาก desktop ของคุณไว้
แล้ว route browser keystroke เข้าไปให้ desktop รัน agent ตามปกติ
และ fan out ทุก assistant delta, tool call, approval prompt กลับ
ผ่าน WS เดิม — เบราว์เซอร์จึงเห็น conversation ที่ stream ออกมา
เรียลไทม์

**LINE OA เป็น launcher ไม่ใช่ chat surface** นี่คือจุดที่คนเข้าใจ
ผิดกันบ่อย การคุยกับ `@thClaws` ไม่ได้คุยกับ agent ของคุณ หน้าที่
ของ OA คือระบุตัวคุณ ส่ง pairing code ให้ บอกว่า desktop ของคุณ
ออนไลน์อยู่ไหม และ mint link ใช้ครั้งเดียวที่พาไปเปิดแชทจริงที่
`chat.thclaws.ai` ตัวบทสนทนาอยู่ในเบราว์เซอร์ LINE เหลืองานเดียว
ใน loop ของการสนทนา คือส่ง approval prompt ให้ตอนที่ไม่มี browser
session เปิดอยู่

## pair มือถือ (LINE)

setup ครั้งเดียว:

1. **เพิ่ม LINE OA** — scan QR ที่
   [`thclaws.ai/line`](https://thclaws.ai/line) (หรือ search
   `@thClaws` ใน LINE)
2. ใน thClaws เปิด Settings → **LINE** → **Pair phone** modal
   จะแสดง code 6 ตัวอักษร (เช่น `KJ4-9P2`)
3. ส่ง code นั้นให้ LINE OA bot จะตอบ "Paired ✓ as
   *<line-display-name-ของคุณ>*"
4. chip LINE บน sidebar เปลี่ยนเป็นเขียว — เชื่อมต่อแล้ว

หลัง pair แล้ว ส่งอะไรให้ OA ก็ได้ มันจะตอบกลับด้วย link ใหม่เข้า
browser chat ซึ่งเป็นที่ที่คุณคุยกับ agent จริง ๆ ส่วน tool call ที่
ต้อง approve (Bash, Edit, Write) จะกลับมาที่ LINE เป็น Quick Reply
chip เมื่อไม่มี browser session เปิดอยู่ — แตะ **[Approve]** หรือ
**[Deny]** จากมือถือ

## OA ตอบอะไรกลับมาบ้าง

OA ไม่ใช่ console สำหรับพิมพ์คำสั่ง ไม่ว่าคุณพิมพ์อะไรลงไป มันจะดู
ข้อเท็จจริงสองอย่าง — pair แล้วหรือยัง และ desktop ออนไลน์อยู่ไหม
ตอนนี้ — แล้วตอบกลับด้วยหนึ่งในสามอย่าง

| สถานะของคุณ | สิ่งที่ตอบกลับ |
|---|---|
| ยังไม่ได้ pair | ข้อความต้อนรับพร้อม pairing code ใหม่ |
| pair แล้ว desktop ออฟไลน์ | "thClaws not active" พร้อมชี้ไปที่ `/pair` ถ้าเครื่องนั้นหายไปถาวร |
| pair แล้ว desktop ออนไลน์ | magic link ใช้ครั้งเดียวเข้า browser chat |

แถวที่สามคือแถวสำคัญ — **เมื่อ pair แล้วและออนไลน์ ทุกข้อความจะได้
link กลับมา** ไม่ว่าจะเป็น `hello`, `/chat`, สติกเกอร์ หรือย่อหน้ายาว ๆ
ที่คุณตั้งใจส่งให้ agent สิ่งที่คุณพิมพ์ใน LINE ไม่ไปถึง agent และไม่ถูก
เก็บเข้าคิวไว้ให้ด้วย ถ้าเผลอพิมพ์ prompt ลงใน LINE ให้เปิด link แล้ว
พิมพ์ใหม่ในเบราว์เซอร์

คำสั่งจริงมีตัวเดียวคือ **`/pair`** ซึ่งบังคับออก pairing code ใหม่ไม่ว่า
คุณจะอยู่ในสถานะไหน เป็นทางออกสำหรับตอนที่ thClaws เครื่องที่ code ผูกอยู่
หายไปแล้วและต้องลงทะเบียนเครื่องใหม่

> `/chat`, `/unpair` และ `/status` **ไม่ใช่** สิ่งที่พิมพ์ใส่ OA แต่เป็น
> HTTP route บน relay ที่ desktop กับเบราว์เซอร์เรียกใช้ พิมพ์ลงไปใน LINE
> ก็ได้แค่ link ตอบกลับตามปกติ release เก่า ๆ เคยใช้ LINE เป็น chat surface
> จริง และเอกสารเก่าบางฉบับยังอธิบายไว้แบบนั้นอยู่

ปุ่ม **Chat** ใน rich menu ทำงานด้วยการส่งข้อความ `/chat` แทนคุณ ซึ่งก็ตกลง
ไปในสาขา "pair แล้วและออนไลน์ → link" อันเดียวกัน มันเป็นทางลัดให้แตะครั้งเดียว
ไม่ใช่คำสั่งคนละตัว

ข้อความที่ไม่ใช่ text (สติกเกอร์ รูป ไฟล์) ระบบไม่เข้าใจ มันจะมาถึงในรูป
ข้อความตรงตัวว่า `(non-text message)` แล้วได้คำตอบเดียวกับอย่างอื่น

## browser chat (เส้นทาง `/chat`)

LINE bubble เหมาะกับ approve สั้น ๆ และ prompt เร็ว ๆ แต่อ่าน
code block, response ยาว ๆ, markdown ลำบาก ส่ง `/chat` ให้ OA
แล้วจะได้ magic link กลับมา:

```
https://chat.thclaws.ai/launch?token=...
```

link นี้ **ใช้ครั้งเดียวและมี TTL 5 นาที** — ให้ขอตอนพร้อมจะเปิดจริง
ไม่ใช่ขอเผื่อไว้ล่วงหน้า (ตัวเลข 10 นาทีที่อาจเคยเห็นคือ idle timeout
ของ *session* ในเบราว์เซอร์ ซึ่งเป็นนาฬิกาคนละเรือนที่เริ่มนับหลังคุณ
เข้าไปแล้ว)

เปิดในเบราว์เซอร์ตัวไหนก็ได้ — link auto-redirect ผ่านหน้า
splash (มีไว้เลี่ยง URL-preview crawler ของ LINE ที่จะใช้ token
ก่อนคุณแตะ) หลัง redirect คุณจะตกลงบน chat surface เต็มรูปแบบ:

- sidebar แสดง session id, ปุ่ม sign-out, และ indicator
  "browser connected" ที่ฝั่ง desktop
- assistant response render เป็น markdown พร้อม code block ที่
  syntax-highlight (ผ่าน marked.js + DOMPurify ที่ vendor มา —
  render ทั้งหมดอยู่ในเบราว์เซอร์ ไม่มี remote loader ไม่มี
  eval)
- history replay อัตโนมัติตอน connect — reconnect กลางคันก็
  resume จากจุดเดิม (~50 message ล่าสุด ส่งจาก Redis stream บน
  relay)
- tool approval เปิด modal inline พร้อมปุ่ม **[Approve] [Deny]**
  แทนที่จะ route ไปที่ LINE Quick Reply
- session หมดอายุหลัง idle 10 นาที — reconnect fail 3 ครั้งติด
  trigger splash "session expired" ชี้กลับไปที่ `/chat` ใน LINE
  เพื่อขอ link ใหม่

browser link เป็น **per-session, single-use, HTTPS-only,
HttpOnly cookie** การแชร์ link เท่ากับยื่น session บน desktop
ให้คนอื่น — อย่าทำ

## rich-menu shortcut (v0.9.3+)

ถ้ามือถือคุณแสดง rich menu ของ LINE OA (toolbar ด้านล่างพร้อม
ปุ่ม custom) จะมีปุ่ม pin ไว้ 2 ปุ่ม:

- **Chat** — เทียบเท่าพิมพ์ `/chat` แตะครั้งเดียวก็ได้ magic
  link ไปยัง browser chat
- **Pair** — เทียบเท่าพิมพ์ `/pair` ใช้ออก pairing code ใหม่
  ไวสุดเวลา disconnect

operator ที่ deploy LINE OA เองสามารถติดตั้ง rich menu ด้วย
script `dev-plan/08-line-server-k3s/rich-menu-setup.sh` — อ่าน
[`docs/line-rich-menu-setup.md`](../../docs/line-rich-menu-setup.md)
สำหรับ walk-through เต็มชุด

## approve จากมือถือหรือเบราว์เซอร์

ตอน LINE bridge connect runtime permission mode จะเป็น `linegated`
(ดู[บทที่ 5](ch05-permissions.md)) และ **ทุก approval request จะ
route ไปที่ LINE ไม่ว่าจะพิมพ์มาจาก surface ไหน** — Terminal tab,
Chat tab, REPL หรือ LINE bubble ก็ตาม approver เป็น process-wide
singleton ไม่รู้ว่า tool call มาจาก surface ไหน — ขณะ pair อยู่
phone คือ approval inbox เดียว

- **browser chat (`/chat`) เปิดอยู่:** modal approval โผล่ขึ้นมา
  ใน browser พร้อมชื่อ tool, preview argument เต็ม ๆ, และปุ่ม
  **[Approve] [Deny]** — UX ดีกว่าให้ Quick Reply chip ของ LINE
  preview argument ยาว ๆ approve ที่ surface ไหนก็ปิดทั้งสอง
- **browser chat ไม่เปิด (หรือยังไม่ได้ mint):** fall back ไปที่
  LINE OA Quick Reply bot push bubble หน้าตาแบบนี้:

  ```
  thClaws wants to run:
    bash -c "ls -la ~/Downloads"

  [Approve]  [Deny]
  ```

  แตะ chip; คำตอบไหลกลับไป desktop ภายใน ~1 วินาที

**bypass ตอน pair อยู่** ถ้าไม่อยากให้ approval route ไปมือถือ:

- `/permissions auto` — override `linegated` ทำให้ mutating tool
  รันได้โดยไม่ขออนุญาต persist ลง `settings.json` ยืนข้าม LINE
  disconnect/reconnect
- disconnect LINE ที่ Settings → LINE Connect — restore pre-LINE
  mode (ปกติคือ `auto` หรือ `ask`) ทันที

ตอน LINE **ไม่ได้** pair modal Approval บน desktop ทำงานปกติ —
routing มือถือ/เบราว์เซอร์ active เฉพาะตอน bridge connect เท่านั้น

## upload ไฟล์จากมือถือหรือเบราว์เซอร์

attach ไฟล์จาก surface ไหนก็ได้ — desktop บันทึกไฟล์ลง
`<workspace>/_uploads/` แล้ว `AGENT.md` ในไดเรกทอรีนั้นบอก agent
ว่าควรทำอะไรกับไฟล์ เพิ่มใน v0.9.6

**ลิมิต:**
- **25 MB ต่อไฟล์** (`UPLOAD_MAX_BYTES`)
- **5 ไฟล์ต่อ message** (`UPLOAD_MAX_FILES`)
- MIME type อะไรก็ได้ — text, image, PDF, archive desktop ไม่
  unpack ไม่ transform แค่ land byte ลง workspace

**ชื่อไฟล์ชน** จะ resolve ด้วยการเติม `_n` ก่อน extension ถ้า
upload `notes.md` แล้วมี `notes.md` อยู่แล้ว ไฟล์ที่สองจะลงเป็น
`notes_1.md`; ตัวที่สามเป็น `notes_2.md` ชื่อ original
preserve ไว้ (ยกเว้น sanitise path-traversal — `../../etc/passwd`
ลงเป็น `passwd`)

**จาก browser chat** (`chat.thclaws.ai`): drag-and-drop ไฟล์ลง
บน chat surface หรือคลิก icon paperclip ข้างช่อง composer
desktop ส่ง synthetic chat message อธิบาย upload (ชื่อไฟล์ +
ขนาด) ตามด้วยบรรทัด directive `Read the file and respond.`
agent จึงตีว่า drop เป็นคำขอให้ลงมือทำกับ content — ไม่ใช่แค่
FYI (ก่อน v0.9.7 synthetic เป็นข้อมูลล้วน บาง model จะตอบ
"อยากให้ทำอะไรกับสิ่งนี้?") project-level `AGENT.md` /
`CLAUDE.md` override directive ได้ ถ้านั่นคือพฤติกรรมที่ต้องการ

**ทำจาก LINE ไม่ได้** การแนบรูปหรือไฟล์ในแชท LINE ไม่เกิดประโยชน์
อะไร เพราะ relay ยุบทุกข้อความที่ไม่ใช่ text ให้เหลือสตริงตรงตัวว่า
`(non-text message)` คุณจึงได้แค่คำตอบ launcher ตามปกติ และไฟล์ไม่เคย
ถูกดึงมาเลย ให้เปิด browser chat แล้ว drop ไฟล์ตรงนั้นแทน (การรองรับ
รูปและเสียงผ่าน LINE อยู่ในแผน ยังไม่ได้ ship)

**คุม behavior ที่ไหน:** วาง `AGENT.md` ไว้ที่
`<workspace>/_uploads/AGENT.md` (หรือที่ workspace root ถ้าอยาก
กฎเดียวสำหรับทุกอย่าง) agent อ่านเป็นส่วนหนึ่งของ cascade
CLAUDE.md / AGENT.md ปกติ แล้วทำตาม directive ที่อยู่ในนั้น:
"OCR PDF ที่ upload ทุกตัวแล้วเก็บ text ใน `kms/sources/`",
"auto-rename screenshot จาก `Photo 2026-…` เป็น slug", ฯลฯ ถ้า
ไม่มี `AGENT.md` ไฟล์จะนิ่งรอจนคุณบอก agent ขั้นต่อไป

## privacy และ trust boundary

- **desktop ไม่ proxy upstream LLM call ผ่าน relay**
  prompt ของคุณวิ่งจาก desktop ตรงไปยัง Anthropic / OpenAI /
  ฯลฯ relay carry แค่ user-facing message ระหว่าง surface กับ
  desktop เท่านั้น
- **relay เห็น content message ระหว่างทาง** (ต้องเห็นเพื่อ
  route) ถ้าไม่อยากให้ third-party อ่าน prompt host เองได้ —
  binary relay อยู่ที่ `crates/line-server/` ใน workspace fork;
  ตัว public OSS ไม่ ship distribution นี้ อ่าน plan-08 ใน
  `dev-plan/` ของ workspace สำหรับ k3s deployment shape
- **token / API key ไม่ออกจาก desktop** relay ถือ LINE channel
  secret หนึ่งตัว (ไว้ verify signature) และ Postgres user
  profile cache (ชื่อ + LINE user id) ต่อ paired user — แค่นั้น
- **LINE pairing token ใช้ครั้งเดียว, TTL 10 นาที, hash ฝั่ง
  server** pairing code ที่ถูกขโมยไปใช้ไม่ได้แล้วหลัง OA emit
  reply "Paired ✓"
- **การ pair มีอายุ 30 วัน** binding token จะหมดอายุเองหลังจากนั้น
  มือถือที่ pair ไว้เมื่อเดือนก่อนแล้วไม่ได้ใช้เลยจึงตกกลับไปได้คำตอบ
  แบบต้อนรับพร้อม code ใหม่ ไม่ใช่ความผิดพลาด — ส่ง `/pair` แล้ว
  redeem code ใหม่ได้เลย

## troubleshooting

| อาการ | สาเหตุที่น่าจะใช่ | วิธีแก้ |
|---|---|---|
| link `/chat` ขึ้น "expired" ตั้งแต่แตะครั้งแรก | URL preview crawler ของ LINE กิน token ไปก่อน | เปิด link จาก LINE chat ตรง ๆ ไม่ใช่จาก copy ที่ forward ต่อ |
| LINE bot ตอบ "thClaws not active" | WS ฝั่ง desktop หลุด (sleep, network) | เปิด desktop กลับมา; pairing ยังอยู่ |
| พิมพ์ prompt ลงใน LINE แล้วได้ link กลับมา | ปกติ — OA เป็น launcher ไม่ใช่ chat surface | เปิด link แล้วพิมพ์ใหม่ในเบราว์เซอร์ |
| browser chat ค้างที่ "Opening thClaws Chat…" | เบราว์เซอร์ block script auto-submit inline | check ว่า CSP อนุญาต `script-src 'self' 'unsafe-inline'` บน `/launch` |
| ปุ่ม Quick Reply ของ LINE ไม่ขึ้นตอน approval | browser chat เปิดอยู่ด้วย — approval ไปที่นั่นแทน | approve ในเบราว์เซอร์ หรือปิด tab แล้ว approval รอบหน้าจะตกไปที่ LINE |
| pairing code ค้าง "(none)" หลังพิมพ์ส่ง | code หมดอายุ หรือถูกใช้ไปแล้ว | เปิด Pair modal อีกครั้งเพื่อ mint code ใหม่ |
| link `/chat` หมดอายุก่อนได้เปิด | TTL ของ magic link คือ 5 นาที ไม่ใช่ 10 | ส่งอะไรให้ OA อีกครั้งเพื่อขอ link ใหม่ |
| pill "browser connected" ไม่โผล่บน desktop | TTL ของ magic link token หมดก่อนคุณเปิด | ส่ง `/chat` จาก LINE ใหม่เพื่อขอ link ใหม่ |

## status command บน desktop

`make line-status` (จาก workspace root) print ตาราง status ต่อ
user โดย join Postgres profile กับ Redis presence flag —
มีประโยชน์สำหรับ operator ที่ run LINE relay ของตัวเอง:

```
$ make line-status
user_id            paired  present  browser  last_seen
U1a2b3...          ✓       ✓        -        2 min ago
U9z8y7...          ✓       -        -        3 days ago
```

`paired` = เคย pair, `present` = WS connect อยู่ตอนนี้,
`browser` = `/chat` browser session active, `last_seen` =
webhook activity ล่าสุด

## ไม่อยู่ในบทนี้

- internal architecture (broker channel multiplex, WS protocol,
  Redis stream layout) — ดู
  [`line-bridge.md`](../../thclaws-technical-manual/line-bridge.md)
  ใน technical manual
- ตั้ง LINE OA ตั้งแต่ต้น (channel secret, webhook URL, ติดตั้ง
  rich menu) — เรื่องฝั่ง operator อ่านได้ที่
  [`docs/line-rich-menu-setup.md`](../../docs/line-rich-menu-setup.md)
  และ doc plan-08 ใน workspace
- cloud gateway (paid SaaS proxy) — ship ใน v0.9.6 ในชื่อ
  `gateway.thclaws.ai` ดู[บทที่ 6](ch06-providers-models-api-keys.md)
  สำหรับ sign-in + toggle ต่อ provider ที่ฝั่ง user, และ
  [`provider-thclaws-gateway.md`](../../thclaws-technical-manual/provider-thclaws-gateway.md)
  ใน technical manual สำหรับ wire shape
