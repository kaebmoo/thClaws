# บทที่ 13 — Hooks

Hooks คือคำสั่งเชลล์ที่ทำงานเมื่อเกิดเหตุการณ์ในวงจรชีวิตของ agent ซึ่งเป็น
ช่องทางให้คุณเชื่อม thClaws เข้ากับเครื่องมือที่มีอยู่ เช่น บันทึกทุกการเรียก tool
ส่งการแจ้งเตือนเมื่อ session จบ หรือบล็อกการ commit จนกว่า test
จะผ่าน

## เหตุการณ์ (Events)

| Event | เกิดขึ้นเมื่อ | Env vars ที่เปิดให้ใช้ |
|---|---|---|
| `pre_tool_use` | ก่อนที่ tool จะทำงาน | `THCLAWS_TOOL_NAME`, `THCLAWS_TOOL_INPUT` |
| `post_tool_use` | หลังจาก tool คืนค่าสำเร็จ | `THCLAWS_TOOL_NAME`, `THCLAWS_TOOL_OUTPUT` |
| `post_tool_use_failure` | หลังจาก tool เกิดข้อผิดพลาด | `THCLAWS_TOOL_NAME`, `THCLAWS_TOOL_ERROR` |
| `permission_denied` | ผู้ใช้พิมพ์ `n` ที่ prompt ขออนุญาตใช้ tool | `THCLAWS_TOOL_NAME` |
| `session_start` | เมื่อ session เริ่มต้น | `THCLAWS_SESSION_ID`, `THCLAWS_MODEL` |
| `session_end` | เมื่อ `/quit` หรือปิดหน้าต่าง | `THCLAWS_SESSION_ID`, `THCLAWS_MODEL` |
| `pre_compact` | ก่อนการบีบอัดประวัติ | — |
| `post_compact` | หลังการบีบอัดเสร็จสิ้น | — |

## การตั้งค่า hooks

ใน `.thclaws/settings.json` (ระดับโปรเจกต์) หรือ
`~/.config/thclaws/settings.json` (ระดับผู้ใช้):

```json
{
  "hooks": {
    "pre_tool_use":  "echo \"tool: $THCLAWS_TOOL_NAME\" >> /tmp/thclaws.log",
    "post_tool_use": "echo \"done: $THCLAWS_TOOL_NAME\" >> /tmp/thclaws.log",
    "session_start": "osascript -e 'display notification \"thClaws started\"'",
    "session_end":   "osascript -e 'display notification \"thClaws ended\"'"
  }
}
```

แต่ละค่าคือ shell snippet ที่รันผ่าน `/bin/sh -c` โดยตัวแปรสภาพแวดล้อม
จะพร้อมให้ใช้งานตามที่ระบุไว้ในตารางด้านบน

## สูตรการใช้งานจริง

### บันทึกทุกคำสั่ง bash ลงไฟล์

```json
{
  "hooks": {
    "pre_tool_use": "[ \"$THCLAWS_TOOL_NAME\" = Bash ] && echo \"[$(date)] $THCLAWS_TOOL_INPUT\" >> ~/.thclaws-bash.log"
  }
}
```

### แจ้งเตือนบนเดสก์ท็อปเมื่อ turn เสร็จ

```json
{
  "hooks": {
    "session_end": "notify-send 'thClaws' 'Session done'"
  }
}
```

สำหรับ macOS: ให้เปลี่ยนเป็น `osascript -e 'display notification "Session done" with title "thClaws"'`

### Auto-commit ทุกครั้งที่ edit สำเร็จ

```json
{
  "hooks": {
    "post_tool_use": "[ \"$THCLAWS_TOOL_NAME\" = Edit -o \"$THCLAWS_TOOL_NAME\" = Write ] && git add -A && git commit -m 'thclaws: edit' --no-verify"
  }
}
```

(ใช้ `--no-verify` อย่างระมัดระวัง เพราะจะข้าม pre-commit hooks ไปด้วย)

### Ping webhook เมื่อถูกปฏิเสธสิทธิ์

```json
{
  "hooks": {
    "permission_denied": "curl -s -X POST -H 'Content-Type: application/json' -d \"{\\\"tool\\\": \\\"$THCLAWS_TOOL_NAME\\\"}\" https://hooks.example.com/denied"
  }
}
```

## `pre_tool_use` gate — บล็อก tool ได้จริง

`pre_tool_use` ทำงาน **แบบ synchronous ก่อน tool** จึง *ปฏิเสธ* การเรียกได้:
**`exit 2` คือบล็อก** (tool ไม่ถูกรันเลย และ **stderr** ของ hook จะถูกส่งให้
โมเดลเป็นเหตุผล) exit code อื่นทั้งหมดคืออนุญาต — hook ที่ทำแค่ audit
(สูตร logging ด้านบน ซึ่ง exit 0) จึงไม่เคยบล็อกอะไร gate นี้ทำงานกับ tool
**ทุกตัว** บนทุก surface และถูกสืบทอดไปยัง subagent กับ workflow ด้วย

**อ่านคำสั่งเต็มจาก stdin** `$THCLAWS_TOOL_INPUT` มีเพดาน (~8 KB) คำสั่งยาวๆ
จึงอาจซ่อนส่วนท้ายไว้พ้นจุดตัดได้ ตัว input เต็มๆ ของ tool (JSON) ที่**ไม่ถูก
ตัด** ถูกส่งมาทาง **stdin** (พร้อมตั้ง `$THCLAWS_TOOL_INPUT_ON_STDIN=1`)
ให้ตรวจจากตัวนั้น:

```json
{
  "hooks": {
    "pre_tool_use": "in=$(cat); [ \"$THCLAWS_TOOL_NAME\" = Bash ] && echo \"$in\" | grep -Eq '> *(/etc|/Users/[^/]+/[.])' && { echo 'write outside the project blocked' >&2; exit 2; }; exit 0",
    "fail_closed": true
  }
}
```

**`fail_closed`** (ค่าเริ่มต้น `false`): เมื่อตั้งเป็น `true` gate จะ **fail
closed** — timeout, spawn ไม่สำเร็จ หรือผลลัพธ์ใดๆ ที่ไม่ใช่ `exit 0`
จะกลายเป็น **ปฏิเสธ** แทนที่จะอนุญาต เปิดเมื่อ hook ตัวนี้*คือ*ขอบเขต
ความปลอดภัยของคุณเอง

> hook ที่คัดกรองข้อความเป็นชั้น **อ่อน** — การอำพราง (`$(printf …)`, `eval`,
> `base64|sh`) หลบการจับคู่ข้อความได้ ถ้าต้องการขอบเขตแบบแข็งที่ OS บังคับว่า
> "ห้ามเขียนนอก workspace" ให้ใช้ **`bash.sandbox`**
> ([บทที่ 5](ch05-permissions.md#bash-sandbox)) รองอยู่ข้างใต้

## การจัดการความล้มเหลว

Hooks ที่จบด้วย exit status ไม่เป็น 0 จะพิมพ์ warning ไปที่ stderr
แต่จะไม่หยุดการทำงานของ agent โดย hook จะรันใน cwd เดียวกับ thClaws
ดังนั้น path ของไฟล์ในสคริปต์จะอิงจากรากของ sandbox ส่วน hooks ที่รันนาน
จะบล็อก turn จึงควรทำให้เร็วหรือสั่งรันเป็น background (`command &`)

## การดีบัก

```bash
thclaws --cli --verbose
```

โหมด verbose จะพิมพ์รายละเอียดของทุก hook ออกมาก่อนจะสั่งรันจริง

> **หมายเหตุการอัปเกรด:** ใน release **ก่อน v0.88.0** hooks ที่ตั้งไว้ใน
> `settings.json` จะไม่ทำงานเลย — key `hooks` ถูกทิ้งเงียบๆ ตอนโหลดไฟล์
> (issue #180) ทั้งระดับ project และ user ทุกโหมด ถ้าคุณเคยตั้ง hooks
> บนเวอร์ชันเก่าแล้วไม่เห็นอะไรทำงาน การตั้งค่าของคุณถูกต้องแล้ว —
> อัปเกรดเป็น ≥ 0.88.0 แล้ว snippet เดิมใช้ได้ทันที Hooks ทำงานทั้งใน
> GUI, CLI, `--serve` และ (ตั้งแต่ release เดียวกัน) โหมด `-p`

## สิ่งที่ hooks ไม่ใช่

Hooks **ไม่สามารถเปลี่ยนแปลง** การเรียก tool ได้ เพราะทำหน้าที่เป็นเพียง observer
เท่านั้น หากต้องการบล็อก tool ให้ใช้รายการ `permissions.deny` (บทที่ 5)
ส่วนการแก้ไข input ของ tool ต้องให้ตัวโมเดลเป็นผู้ทำเอง
