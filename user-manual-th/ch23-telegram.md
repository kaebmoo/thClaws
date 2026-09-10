# บทที่ 23 — Telegram bot

ขับ thClaws จาก Telegram สร้าง bot ด้วย `@BotFather` เอา token มา
วางใน thClaws แล้วทุก message ที่คุณ DM หา bot จะรันเป็น turn บน
desktop ของคุณ — tool registry เต็มชุด (Bash, Edit, KMS, MCP,
skills) รันในเครื่อง แล้ว stream คำตอบกลับมาเป็น Telegram message
tool call ที่ต้อง approve จะโผล่เป็นปุ่ม inline keyboard ให้แตะจาก
มือถือ (dev-plan/29)

## ทำไมเป็น Telegram (และต่างจาก LINE ยังไง)

[LINE bridge](ch21-line-and-browser-chat.md) ต้องมี relay server
(`line.thclaws.ai`) เพราะ LINE ส่ง message ด้วยการ push HTTPS
webhook ไปที่ endpoint ที่ต้องมีคน host เท่านั้น แต่ Bot API ของ
Telegram เปิด **long-polling** (`getUpdates`) ซึ่งทำงานหลัง NAT
ได้สบาย — thClaws เลยคุยกับ `api.telegram.org` **โดยตรง** ไม่ต้องมี
relay ไม่ต้องรัน server ไม่มี third party คั่นกลางใน message path
และนอกประเทศไทย Telegram ก็เป็น default ที่คนใช้มากกว่า

desktop ไม่หายไปไหน — code, secret, tool ทั้งหมดยังอยู่ในเครื่อง
Telegram เป็นแค่ surface สำหรับ chat เท่านั้น

## ทำงานยังไง (พารากราฟเดียว)

ตอน connect thClaws จะเปิด long-poll loop ไปที่ `api.telegram.org`
ดึง message ใหม่มาเรื่อย ๆ ตามที่เข้ามา message ที่ผ่านการ
authorize จะถูกป้อนให้ agent บน desktop แล้ว assistant text สุดท้าย
จะถูก HTML-escape, ตัดเป็นชิ้นให้พอดีลิมิต 4096 ตัวอักษรของ
Telegram แล้วส่งกลับด้วย `sendMessage` tool ที่แก้ไขสถานะระบบจะหยุด
turn ไว้แล้วโพสต์ inline keyboard (**Allow / Always / Deny**) การ
แตะของคุณจะปลดล็อก gate แล้วแก้ message เดิมให้แสดงผลลัพธ์ที่เลือก

## การตั้งค่า

### 1. สร้าง bot

ใน Telegram เปิด chat กับ **`@BotFather`** แล้วส่ง `/newbot` ทำตาม
ขั้นตอน (ตั้งชื่อและ username ที่ลงท้ายด้วย `bot`) BotFather จะตอบ
**token** กลับมาหน้าตาแบบนี้:

```
<your-bot-id>:<token-from-botfather>
```

(รูปแบบ: ตัวเลข แล้วตามด้วย `:` แล้วตัวอักษรประมาณ 35 ตัวจากชุด `[A-Za-z0-9_-]`)

เก็บเป็นความลับ — token คือ API key เต็มของ bot

### 2a. connect จาก GUI

1. เปิด **Settings → Telegram Connect…**
2. วาง bot token แล้วกด **Connect** thClaws จะ validate กับ Telegram
   (`getMe`) แล้วเริ่ม poll; sidebar จะแสดง pill **Telegram** พร้อม
   `@username` ของ bot
3. **กด approve ที่ thClaws ก่อน** การ connect เริ่ม bridge แล้วก็จริง
   แต่ยังไม่มีใครอยู่ใน allowlist (DM policy เริ่มต้นคือ `pairing`) bot
   จึงยังไม่ตอบใคร — *รวมถึงตัวคุณเอง* — จนกว่าจะ approve ให้ DM หา bot
   จากมือถือ จะมี **pairing request** โผล่ขึ้นใน Telegram Connect modal
   พร้อมปุ่ม **Approve** กดปุ่มนั้นแล้วถึงจะเริ่มคุยได้ ดูหัวข้อ
   **Pairing** ด้านล่างสำหรับ flow เต็ม

### 2b. …หรือรันแบบ headless

ไม่ต้องใช้ GUI — ตั้ง token ใน environment แล้วสั่งรัน bot loop:

```bash
export TELEGRAM_BOT_TOKEN="123456789:AA…"
export TELEGRAM_OWNER_ID="<Telegram user id ที่เป็นตัวเลขของคุณ>"   # ไม่บังคับแต่แนะนำ
thclaws --telegram
```

`--telegram` รัน agent loop ของตัวเอง (GUI worker ใช้ได้เฉพาะบน
desktop) พิมพ์ `connected as @yourbot` แล้วเสิร์ฟ message จนกด
Ctrl-C ใช้ `.thclaws/settings.json` ของโปรเจกต์เดียวกับ REPL

**การ approve tool call ในโหมด headless** ค่าเริ่มต้น bot จะรันแบบ
*gated* — ทุก tool call ที่แก้ไขข้อมูลจะส่งปุ่ม Approve/Deny เข้ามาใน
แชต ถ้า VPS เล็กและอยากให้ agent รันเองไม่ต้องคอยกด ให้สลับเป็น
**auto** (ไม่ถาม) ได้หลายวิธี ซึ่ง `--telegram` รองรับทั้งหมด:

```bash
thclaws --telegram --accept-all            # ครั้งเดียว: รันรอบนี้แบบไม่ถาม
thclaws --telegram --permission-mode auto  # เหมือนกัน แต่ระบุชัด
```

…หรือตั้งให้ถาวรทุกครั้งที่รัน โดยใส่ใน `.thclaws/settings.json`
(โฟลเดอร์ที่คุณสั่งรัน bot):

```json
{ "permissions": "auto" }
```

> auto = agent รันทุก tool โดยไม่ถาม เปิดเฉพาะ bot ที่คุณไว้ใจให้ทำงาน
> เองได้ การพิมพ์ `/permissions auto` ใน `thclaws --cli` อีกหน้าต่างก็
> เขียนค่านี้ลง `.thclaws/settings.json` ด้วย ดังนั้น `thclaws
> --telegram` ที่รันจาก**โฟลเดอร์เดียวกัน**ภายหลังจะใช้ค่านี้ (build
> เก่ามากๆ อาจไม่เซฟคำสั่งจาก CLI — ถ้าของคุณไม่เซฟ ให้แก้
> `settings.json` ตรงๆ หรือใช้ `--accept-all`)

> **หา user id ของตัวเอง:** ทัก `@userinfobot` (หรือ bot "what's my
> id" ตัวไหนก็ได้) บน Telegram `TELEGRAM_OWNER_ID` จะเพิ่มคุณเข้า
> allowlist ตั้งแต่ตอน start ทำให้ DM bot ได้ทันที — headless ไม่มี
> GUI ให้กด approve pairing code (ดูด้านล่าง)

## Pairing — ใครได้รับอนุญาตให้คุยกับ bot

ใครก็ตามที่รู้ `@username` ของ bot ทัก bot ได้ ฉะนั้นโดย default
thClaws จะ **ไม่** ตอบคนแปลกหน้า DM policy เริ่มต้นคือ `pairing`:

1. user ใหม่ DM หา bot
2. bot ตอบ: *"คุณยังไม่ได้ pair รหัส pairing ของคุณคือ `123456`
   ขอให้เจ้าของ thClaws approve ให้"*
3. ใน **Settings → Telegram Connect…** เจ้าของจะเห็น request (ชื่อ +
   รหัส) พร้อมปุ่ม **Approve** / **Reject**
4. พอกด **Approve** user id จะถูกเพิ่มเข้า `allowFrom` เซฟลงดิสก์
   แล้ว bot จะ DM ไปว่า *"You're approved!"*

รหัสหมดอายุใน **1 ชั่วโมง** ในโหมด headless ไม่มี GUI ให้กด approve —
ใช้ `TELEGRAM_OWNER_ID` (เข้า allowlist ทันที) หรือใส่ `allowFrom` ใน
ไฟล์ config ไว้ล่วงหน้า

ตั้ง `dmPolicy: "allowlist"` แทนถ้าอยากให้เพิกเฉยคนแปลกหน้าเงียบ ๆ
โดยไม่มี pairing prompt เลย

## approve tool call จากมือถือ

ขณะที่ Telegram connect อยู่ permission mode ตอนรันคือ
`telegramgated` (ดู [บทที่ 5](ch05-permissions.md)) — ความหมาย
เหมือน `ask` แต่ approval prompt **ทุกอัน** จะ route ไปที่ Telegram
chat ของคุณ ไม่ว่าจะพิมพ์ request ต้นทางจาก surface ไหน (Terminal,
Chat, REPL, Telegram) bot จะโพสต์:

```
🔐 thClaws wants to run: Bash

Input: {"command":"ls -la ~/Downloads"}

Tap a button (auto-denies in 60s).
[ ✅ Allow ] [ ♾️ Always ] [ 🚫 Deny ]
```

- **Allow** — รันครั้งนี้ครั้งเดียว
- **Always** — รันครั้งนี้และทุกครั้งถัด ๆ ไปใน session นี้ (= "allow
  for session")
- **Deny** — agent ได้รับการปฏิเสธแล้วทำ turn ต่อ

หลังแตะ ปุ่มจะหายไปและ message ถูกเขียนใหม่ให้แสดงผลลัพธ์ ไม่แตะ
ภายใน **60 วินาที** จะ auto-deny พิมพ์ `approve` / `deny` แทนการแตะ
ก็ได้

**อยากเลิกให้ approval route ไป Telegram:** บน GUI ให้ disconnect (คืน
ค่า mode `auto` / `ask` ก่อน connect) ส่วน bot แบบ **headless** ให้ตั้ง
`auto` — `thclaws --telegram --accept-all` หรือใส่ `"permissions":
"auto"` ใน `.thclaws/settings.json` (ดูหัวข้อ *การ approve tool call
ในโหมด headless* ด้านบน) โหมด auto จะรันทุก tool โดยไม่ถาม

## กลุ่ม (group)

เพิ่ม bot เข้ากลุ่ม โดย default (`groupPolicy: "allowlist"`) bot จะ
เพิกเฉยกลุ่มนั้นจนกว่าคุณจะ opt-in เพิ่ม chat id ของกลุ่ม (เลขจำนวน
เต็มติดลบ) ใต้ `groups` ใน config หรือตั้ง `groupPolicy: "open"` เพื่อ
เสิร์ฟทุกกลุ่มที่ bot ถูกเพิ่มเข้าไป กลุ่มธรรมดาใช้ session เดียวร่วมกัน
ไม่แยกราย user ทุกคนในห้องจึงคุยอยู่กับบทสนทนาเดียวกัน ยกเว้นกลุ่มแบบ
**forum** ที่แยก session ตาม topic (ดู [Channel และ forum topic](#channel-และ-forum-topic))

> bot ใน Telegram โดย default จะรับ message ในกลุ่มเฉพาะตอนถูก
> mention หรือเป็น command ("privacy mode") ปรับใน BotFather
> (`/setprivacy`) ถ้าอยากให้ bot เห็น text ทั้งหมดในกลุ่ม

## Channel และ forum topic

**channel** ของ Telegram คือรูปแบบกระจายเสียง — agent โพสต์ คนอ่าน
คอมเมนต์ แต่คอมเมนต์ไม่ได้มาที่ channel Telegram จะ route มันเข้า
**linked discussion group** ของ channel นั้น ซึ่งมาถึงในรูปข้อความ
ธรรมดาที่ bot รับมืออยู่แล้ว ผูกทั้งคู่ไว้ใต้ `channels` แล้วทำงานทั้ง
สองฝั่ง

```json
"channels": {
  "-1009876543210": {
    "linkedDiscussionGroup": "-1001111111111",
    "agentId": "researcher"
  }
}
```

bot ต้องเป็น **admin ที่มีสิทธิ์โพสต์** บน channel นั้น thClaws จะ probe
ให้ตอนผูก และรายงาน error ที่ชัดเจน แทนที่จะปล่อยให้การโพสต์ทีหลังพัง
ด้วย 403 เงียบ ๆ

`agentId` อ้างถึง agent def ใต้ `.thclaws/agents/` — คีย์เดียวกับที่ Agent
Teams ใช้ ([บทที่ 17](ch17-agent-teams.md)) เป็นตัวกำหนดว่า agent ตัวไหน
ตอบใน channel นั้นและใน discussion group ของมัน ถ้าไม่ตั้งจะตกกลับไปใช้
agent หลักของคุณ

### Forum topic

supergroup ที่เปิด **Topics** จะถูกแบ่งเป็น thread และ discussion group
ของ channel ก็เป็น forum ได้ คอมเมนต์บนโพสต์หนึ่งจึงลงไปอยู่ใน topic
หนึ่งโดยเฉพาะ ตามมาด้วยสองเรื่อง

- **แต่ละ topic มีบทสนทนาของตัวเอง** session ถูก key ตาม topic สอง topic
  ในกลุ่มเดียวกันจึงไม่ปนบริบทกัน
- **แต่ละ topic มี agent ของตัวเองได้** ผ่าน `topicRouting` ที่ key ด้วย
  เลข topic id ส่วน topic ที่ไม่ได้ระบุจะตกกลับไปใช้ `agentId` ของ channel

```json
"topicRouting": { "42": { "agentId": "support" } }
```

topic **General** คือ id `1` และมีความประหลาดนิดหน่อย: ขามาข้อความของมัน
ไม่มี thread id ติดมาเลย ส่วนขาออก Telegram *ปฏิเสธ* thread id ที่เป็น `1`
thClaws จัดการให้ทั้งสองกรณีแล้ว — คุณอ้างถึง General ด้วยเลข `1` ใน config
ได้ตามปกติ

## การตอบแบบ streaming

ค่าเริ่มต้นคือหนึ่ง turn ได้หนึ่งข้อความตอนจบ ถ้าเปิด `streamPreview`
bot จะโพสต์ placeholder ไว้ก่อนแล้ว **แก้ข้อความนั้นสด ๆ** ระหว่าง agent
generate คุณจึงเห็นคำตอบทยอยมา

```json
{ "streamPreview": true }
```

มีสองข้อจำกัดที่ควรรู้ก่อนเปิด Telegram throttle การแก้ข้อความเดิมซ้ำ ๆ
อย่างหนัก ระบบจึงรวบการแก้ให้เหลืออย่างมาก **1 ครั้งต่อ 1.2 วินาที** และ
ข้ามการแก้ที่เนื้อหาไม่เปลี่ยน อีกข้อคือมีเฉพาะ path headless `--telegram`
ที่รองรับ flag นี้ตอนนี้ — path ที่ผ่าน GUI worker ยังตอบทีเดียวตอนจบ
เมื่อ turn จบ preview จะถูกแทนที่ด้วยคำตอบสุดท้ายที่จัดรูปแบบและตัดชิ้น
เรียบร้อยแล้ว

## การตั้งค่า (Configuration)

state ตอนรันอยู่ใน **`./.thclaws/telegram.json`** — เป็นระดับโปรเจกต์
resolve จากไดเรกทอรีที่คุณเปิด thClaws และเขียนโดย GUI modal แต่ละ
โปรเจกต์ (และแต่ละ GUI Shell) จึงเป็นเจ้าของ bot ของตัวเองแยกกัน

> **เดิมอยู่ที่ `~/.config/thclaws/telegram.json`** ตอนนี้ path ระดับ
> user เป็น legacy แล้ว จะถูกอ่านเป็น fallback เท่านั้น และเฉพาะเมื่อ
> ตั้ง `THCLAWS_TELEGRAM_USER_CONFIG=1` ระบบไม่ย้ายและไม่ลบให้อัตโนมัติ
> ถ้าอัปเกรดแล้ว bot หายไป ให้ย้าย `telegram.json` เดิมเข้าไปในโฟลเดอร์
> `.thclaws/` ของโปรเจกต์ (หรือตั้ง env ตัวนั้นไว้ก่อนระหว่างรอย้าย)
> การแก้ไฟล์ระดับ user โดยไม่เปิด opt-in จะไม่มีผลอะไรเลย

โปรเจกต์ใส่ block `telegram` ใน `.thclaws/settings.json` ก็ได้ ฟิลด์:

```json
{
  "enabled": true,
  "botToken": "123456789:AA…",
  "dmPolicy": "pairing",
  "allowFrom": ["111111111"],
  "groupPolicy": "allowlist",
  "groups": { "-1001234567890": { "label": "Team room" } },
  "channels": {
    "-1009876543210": {
      "linkedDiscussionGroup": "-1001111111111",
      "agentId": "researcher",
      "topicRouting": { "42": { "agentId": "support" } }
    }
  },
  "outputCeiling": 4000,
  "streamPreview": false
}
```

| ฟิลด์ | ความหมาย |
|---|---|
| `enabled` | auto-reconnect ตอน launch เมื่อเป็น `true` และมี token |
| `botToken` | token จาก BotFather **ไม่บังคับ** — env `TELEGRAM_BOT_TOKEN` ชนะค่านี้ |
| `dmPolicy` | `pairing` (default) หรือ `allowlist` |
| `allowFrom` | Telegram user id (string) ที่อนุญาตให้ DM |
| `groupPolicy` | `allowlist` (default) หรือ `open` |
| `groups` | chat id ของกลุ่มที่ allowlist → `{ label? }` |
| `channels` | การผูก broadcast channel → `{ linkedDiscussionGroup?, agentId?, topicRouting? }` — ดู [Channel และ forum topic](#channel-และ-forum-topic) |
| `outputCeiling` | ลิมิตตัวอักษรต่อ message ก่อนตัดเป็นชิ้น (default 4000) |
| `streamPreview` | แก้ข้อความเดิมสด ๆ ระหว่าง agent generate แทนการตอบทีเดียวตอนจบ ปิดไว้เป็นค่าเริ่มต้น ใช้ได้เฉพาะ headless `--telegram` |

**ลำดับความสำคัญของ token:** env `TELEGRAM_BOT_TOKEN` → `botToken` ใน
ไฟล์ → ไม่มี การที่ env ชนะหมายความว่าคุณไม่ต้อง commit token ลงดิสก์
สำหรับงาน CI / container การตรวจก่อน upload จะปฏิเสธ token ที่ฝังมา
ใน config ที่ deploy

## CLI

```
thclaws --telegram          รัน bot แบบ headless จนกด Ctrl-C
thclaws telegram status     พิมพ์ config ที่ resolve แล้ว (ปิดบัง token)
thclaws telegram pair       พิมพ์วิธีตั้งค่ากับ @BotFather
```

`telegram status` ใช้เช็กว่า token ถูกตรวจเจอไหม:

```
$ thclaws telegram status
Telegram adapter status
  enabled:        true
  bot token:      123456789:<redacted> (present)
  dm policy:      Pairing
  group policy:   Allowlist
  allow_from:     1 user(s)
  groups:         0 allowlisted
  output ceiling: 4000 chars
```

## การจัดรูปแบบ output

- ส่งคำตอบใน **HTML parse mode** — escape แค่ `<`, `>`, `&` (foot-gun
  น้อยกว่า MarkdownV2 มาก) fenced code block กลายเป็น `<pre>` block
  ส่วน markdown อื่น ๆ แสดงตามตัวอักษร
- คำตอบยาว ๆ ถูกแบ่งเป็นหลาย message ให้ต่ำกว่า `outputCeiling`
  (default 4000) ตัวอักษร ตัดที่ขอบบรรทัดเท่าที่ทำได้ รักษา UTF-8 —
  ภาษาไทย, emoji, CJK ไม่ถูกตัดกลางตัวอักษร
- ANSI escape sequence และบรรทัด tool-call narration ของ GUI
  (บรรทัด `⏺`/`🔧`) ถูกตัดทิ้งก่อนส่ง

## ความเป็นส่วนตัวและขอบเขตความเชื่อใจ

- **ไม่มี relay** thClaws คุยกับ `api.telegram.org` ตรง ๆ ไม่มีใคร
  นอกจาก Telegram กับ desktop ของคุณใน message path
- **token คือกุญแจ** ใครมี token ก็ขับ API ของ bot ได้ ควรใช้
  `TELEGRAM_BOT_TOKEN` (env) มากกว่าเขียนลง `telegram.json` ส่วนการ
  เก็บใน keychain จะมาใน tier ถัดไป
- **LLM call ขาออกไม่ผ่าน Telegram** prompt ของคุณไป desktop →
  Anthropic / OpenAI / ฯลฯ โดยตรง Telegram แค่ขนตัว chat text
- **pairing code อยู่ใน memory, TTL 1 ชั่วโมง** restart process แล้ว
  code ที่ค้างจะหาย (DM ใหม่เพื่อขอ code ใหม่) user ที่ approve แล้ว
  ค้างอยู่ใน `allowFrom`

## อะไรรองรับแล้ว อะไรยังไม่รองรับ

ที่ ship ไปแล้วเกินจาก Tier 1 เดิม (DM + กลุ่ม + text + pairing +
approval แบบ inline keyboard)

- **broadcast channel + linked discussion group + forum-topic routing**
  รวมถึง agent ราย topic และการแยก session ราย topic — ดูข้างบน
- **streaming preview edit** (`streamPreview` เฉพาะ headless) — ดูข้างบน
- **รูปภาพขาเข้า** ส่งรูปให้ bot แล้วมันจะดาวน์โหลดมาส่งให้โมเดลเป็น
  image โดยใช้ caption เป็นข้อความ ไฟล์แนบถูกจำกัดที่ **8 MB** เพราะ
  bytes จะถูก base64 เข้าไปใน prompt รูปใหญ่เกินไปจึงเปลืองโทเคนและอาจ
  ชนลิมิตของ provider ก่อนที่จะช่วยอะไรได้ ถ้าดาวน์โหลดพลาด turn ยังรัน
  ต่อแบบ text อย่างเดียวและบอกเหตุผล ไม่ได้กลืนข้อความคุณทิ้ง

ที่ยังไม่รองรับ — จะถูกเพิกเฉย

- **voice message, document และ sticker** อ่านได้เฉพาะรูปภาพ
- **สื่อขาออก** bot ส่งข้อความอย่างเดียว ไม่ส่งไฟล์หรือรูปกลับให้
- **webhook mode, multi-account, proxy support** ใช้ long-polling
  อย่างเดียว และหนึ่ง bot ต่อหนึ่ง thClaws

## การแก้ปัญหา

| อาการ | สาเหตุที่น่าจะเป็น | วิธีแก้ |
|---|---|---|
| "token rejected by Telegram (401)" ตอน connect | token ผิด/หมดอายุ | copy ใหม่จาก `@BotFather` เช็ก space ท้ายบรรทัด |
| bot ไม่ตอบ DM ของคุณ | คุณยังไม่ได้อยู่ใน allowlist | approve pairing code ใน GUI หรือตั้ง `TELEGRAM_OWNER_ID` (headless) |
| bot เพิกเฉย message ในกลุ่ม | กลุ่มไม่ได้ allowlist หรือ privacy mode ของ BotFather เปิดอยู่ | เพิ่ม chat id ใน `groups` (หรือ `groupPolicy: "open"`); `/setprivacy` ใน BotFather |
| ปุ่ม approval หมดเวลา | ไม่แตะภายใน 60 วินาที | แตะใหม่ที่ request ใหม่ หรือพิมพ์ `approve` |
| headless: เตือน "no allowlisted users yet" | `allowFrom` ว่างและไม่มี owner id | `export TELEGRAM_OWNER_ID=<your id>` แล้ว restart |
| message เก่า replay ตอน start | — | ไม่เกิด: backlog ก่อน launch ถูก drain ทิ้งโดยตั้งใจ |
| bot สอง bot ตีกัน ("Conflict") | มี `getUpdates`/webhook อีกตัวรันด้วย token เดียวกัน | รัน thClaws (หรือ webhook) ตัวเดียวต่อ token |

## สิ่งที่ไม่อยู่ในบทนี้

- สถาปัตยกรรมภายใน (wire type, long-poll loop, approver state
  machine, pairing manager) — ดู
  [`telegram-bridge.md`](../../thclaws-technical-manual/telegram-bridge.md)
  ในคู่มือเทคนิค
- LINE bridge และ browser chat — [บทที่ 21](ch21-line-and-browser-chat.md)
