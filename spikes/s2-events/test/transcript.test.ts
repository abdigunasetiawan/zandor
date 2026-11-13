// Test S2 dengan rekaman event NYATA (fixtures/recording, opencode 1.19.5 + opencode/big-pickle).
import { describe, expect, test } from "bun:test";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Event, ToolPart } from "@opencode-ai/sdk/v2/types";
import {
  HANDLED_EVENTS,
  IGNORED_FOR_TRANSCRIPT,
  isV2Family,
  orderedMessages,
  permissionsForCall,
  TranscriptStore,
} from "../src/reducer";
import { renderDocument, renderTranscript } from "../src/render";

const dir = join(import.meta.dir, "..", "fixtures", "recording");
const events: { t: number; ev: Event }[] = readFileSync(join(dir, "events.ndjson"), "utf8")
  .trim()
  .split("\n")
  .map((l) => JSON.parse(l));
const serverMessages: { info: any; parts: any[] }[] = JSON.parse(readFileSync(join(dir, "messages.json"), "utf8"));
const meta = JSON.parse(readFileSync(join(dir, "meta.json"), "utf8"));

function replay(upTo = events.length) {
  const s = new TranscriptStore();
  for (const { ev } of events.slice(0, upTo)) s.apply(ev);
  return s;
}

describe("rekaman nyata", () => {
  test("fixture mencakup part & tool yang diminta S2", () => {
    for (const t of ["text", "reasoning", "file", "tool", "patch", "step-start", "step-finish"]) expect(meta.partTypes[t]).toBeGreaterThan(0);
    for (const t of ["read:completed", "edit:completed", "bash:completed", "bash:error", "todowrite:completed"]) expect(meta.toolNames).toContain(t);
    expect(meta.permissionLog.map((p: any) => p.reply)).toEqual(["once", "once", "reject"]);
  });

  test("hasil reduce event == GET /session/:id/message (kebenaran server)", () => {
    const store = replay();
    const s = store.sessions.get(meta.sessionID)!;
    const ours = orderedMessages(s);
    expect(ours.length).toBe(serverMessages.length);
    for (let i = 0; i < serverMessages.length; i++) {
      expect(ours[i].info).toEqual(serverMessages[i].info);
      expect(ours[i].parts).toEqual(serverMessages[i].parts);
    }
  });

  test("tidak ada event yang tidak tertangani", () => {
    expect(Object.fromEntries(replay().unhandled)).toEqual({});
  });

  test("delta streaming membangun teks yang sama dengan hasil akhir", () => {
    const store = new TranscriptStore();
    let checked = 0;
    for (const { ev } of events) {
      if (ev.type === "message.part.updated") {
        const p = ev.properties.part;
        if ((p.type === "text" || p.type === "reasoning") && p.time?.end) {
          const before = store.sessions.get(p.sessionID)?.messages.get(p.messageID)?.parts.get(p.id) as any;
          if (before && before.text.length > 0) {
            expect(before.text).toBe(p.text);
            checked++;
          }
        }
      }
      store.apply(ev);
    }
    expect(checked).toBeGreaterThanOrEqual(4); // reasoning + text di beberapa langkah
  });

  test("izin terhubung ke tool call; reject → tool error", () => {
    const s = replay().sessions.get(meta.sessionID)!;
    const tools = orderedMessages(s).flatMap((m) => m.parts.filter((p): p is ToolPart => p.type === "tool"));
    const edit = tools.find((t) => t.tool === "edit")!;
    expect(permissionsForCall(s, edit.callID).map((p) => [p.permission, p.reply])).toEqual([["edit", "once"]]);
    const bashOk = tools.find((t) => t.tool === "bash" && t.state.status === "completed")!;
    expect(permissionsForCall(s, bashOk.callID).map((p) => p.reply)).toEqual(["once"]);
    const bashRejected = tools.find((t) => t.tool === "bash" && t.state.status === "error")!;
    expect(permissionsForCall(s, bashRejected.callID).map((p) => p.reply)).toEqual(["reject"]);
    expect([...s.permissions.values()].every((p) => p.reply)).toBe(true);
  });

  test("render transcript lengkap dari event nyata", () => {
    const s = replay().sessions.get(meta.sessionID)!;
    const html = renderTranscript(s);
    expect(html).toContain('class="part reasoning"');
    expect(html).toContain('class="part file"');
    expect(html).toContain('<span class="tool-name">read</span>');
    expect(html).toContain('<span class="tool-name">edit</span>');
    expect(html).toContain('class="diff"'); // diff edit dari metadata
    expect(html).toContain('<span class="tool-name">bash</span>');
    expect(html).toContain('class="term"'); // output perintah
    expect(html).toContain('<span class="tool-name">todowrite</span>');
    expect(html).toContain('class="part patch"');
    expect(html).toContain("perm-reject");
    expect(html).toContain("perm-once");
    expect(html).toContain("big-pickle");
    expect(html).not.toContain("undefined");
    writeFileSync(join(dir, "..", "transcript.html"), renderDocument(s, "Zandor S2 — transcript dari event nyata"));
    expect(html).toMatchSnapshot();
  });

  test("render di tengah streaming: thinking masih terbuka dan parsial", () => {
    const idx = events.findIndex(({ ev }) => ev.type === "message.part.delta");
    const s = replay(idx + 3).sessions.get(meta.sessionID)!;
    const html = renderTranscript(s);
    expect(html).toContain("💭 Thinking …");
    expect(html).toContain("<details class=\"part reasoning\" open>");
  });
});

describe("kelengkapan terhadap skema SDK 1.19.5", () => {
  const types = readFileSync(join(import.meta.dir, "..", "node_modules/@opencode-ai/sdk/dist/v2/gen/types.gen.d.ts"), "utf8");
  const union = types.match(/^export type Event = ([^;]+);/m)![1].split("|").map((s) => s.trim());
  const eventTypes = union.map((name) => {
    const block = types.match(new RegExp(`^export type ${name} = \\{[\\s\\S]*?\\n\\};`, "m"))![0];
    return block.match(/type: "([^"]+)"/)![1];
  });

  test("setiap jenis event di skema diklasifikasikan (ditangani / milik panel lain / v2)", () => {
    const unclassified = eventTypes.filter((t) => !HANDLED_EVENTS.has(t) && !IGNORED_FOR_TRANSCRIPT.has(t) && !isV2Family(t));
    expect(unclassified).toEqual([]);
  });

  test("keluarga v2 ada di skema tetapi tidak dipancarkan /event pada rekaman", () => {
    const v2 = eventTypes.filter(isV2Family);
    expect(v2.length).toBeGreaterThan(30);
    expect(events.filter(({ ev }) => isV2Family(ev.type))).toEqual([]);
  });
});

describe("unit", () => {
  test("delta sebelum part dikenal ditahan lalu diterapkan", () => {
    const s = new TranscriptStore();
    const base = { sessionID: "ses_1", messageID: "msg_1" };
    s.apply({ id: "evt_1", type: "message.part.delta", properties: { ...base, partID: "prt_1", field: "text", delta: "Halo " } } as Event);
    s.apply({ id: "evt_2", type: "message.part.updated", properties: { sessionID: "ses_1", time: 1, part: { id: "prt_1", ...base, type: "text", text: "" } } } as Event);
    s.apply({ id: "evt_3", type: "message.part.delta", properties: { ...base, partID: "prt_1", field: "text", delta: "dunia" } } as Event);
    expect((s.sessions.get("ses_1")!.messages.get("msg_1")!.parts.get("prt_1") as any).text).toBe("Halo dunia");
  });

  test("izin tanpa tool call tetap tampil sebagai menunggu", () => {
    const s = new TranscriptStore();
    s.apply({ id: "e", type: "permission.asked", properties: { id: "per_1", sessionID: "ses_1", permission: "external_directory", patterns: ["/tmp/*"], metadata: {}, always: [] } } as Event);
    expect(renderTranscript(s.sessions.get("ses_1")!)).toContain("menunggu persetujuan");
  });

  test("event tak dikenal tercatat, tidak melempar", () => {
    const s = new TranscriptStore();
    s.apply({ type: "sesuatu.baru", properties: {} });
    expect(s.unhandled.get("sesuatu.baru")).toBe(1);
  });
});
