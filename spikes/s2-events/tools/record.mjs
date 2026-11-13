#!/usr/bin/env node
// Spike S2 — rekam event SSE nyata dari `opencode serve` untuk skenario yang memicu
// reasoning, baca file, lampiran file, edit (patch), bash, izin (once + reject), dan todo.
//
// Pemakaian: node spikes/s2-events/tools/record.mjs --model opencode/big-pickle [--out <dir>]
// Hasil:  <out>/events.ndjson     — setiap event SSE apa adanya + waktu terima (ms)
//         <out>/messages.json     — GET /session/:id/message di akhir (kebenaran server)
//         <out>/meta.json         — versi, model, skenario, statistik
// Data opencode diisolasi (XDG_DATA_HOME sementara) supaya sesi uji tidak masuk riwayat user.
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync, rmSync, realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { randomBytes } from "node:crypto";

const here = dirname(fileURLToPath(import.meta.url));
const arg = (n, d) => { const i = process.argv.indexOf(n); return i > 0 ? process.argv[i + 1] : d; };
const [providerID, modelID] = arg("--model", "opencode/big-pickle").split("/");
const out = resolve(arg("--out", join(here, "..", "fixtures", "recording")));
const bin = resolve(here, "..", "..", "target", "debug", process.platform === "win32" ? "opencode.exe" : "opencode");

// --- workspace uji ---
const ws = mkdtempSync(join(tmpdir(), "zandor-s2-ws-"));
const data = mkdtempSync(join(tmpdir(), "zandor-s2-data-"));
writeFileSync(join(ws, "add.js"), "// Menjumlahkan dua angka\nfunction add(a, b) {\n  return a - b;\n}\nconsole.log(add(2, 3));\n");
writeFileSync(join(ws, "catatan.txt"), "Proyek uji Zandor S2. Fungsi add() harus mengembalikan a + b.\n");
writeFileSync(join(ws, "sampah.txt"), "boleh dihapus\n");
// Repo git → opencode membuat snapshot & part `patch`.
const git = (...a) => new Promise((res, rej) => spawn("git", a, { cwd: ws, stdio: "ignore" }).on("exit", (c) => (c ? rej(new Error("git " + a.join(" "))) : res())));
writeFileSync(join(ws, "opencode.json"), JSON.stringify({ $schema: "https://opencode.ai/config.json", permission: { edit: "ask", bash: "ask" } }, null, 2));

await git("init", "-q");
await git("-c", "user.email=s2@zandor.invalid", "-c", "user.name=s2", "add", "-A");
await git("-c", "user.email=s2@zandor.invalid", "-c", "user.name=s2", "commit", "-qm", "awal");

const password = randomBytes(24).toString("hex");
const auth = { Authorization: "Basic " + Buffer.from("opencode:" + password).toString("base64") };
const proc = spawn(bin, ["serve", "--hostname", "127.0.0.1", "--port", "0"], {
  cwd: ws,
  env: { ...process.env, OPENCODE_SERVER_PASSWORD: password, OPENCODE_DISABLE_AUTOUPDATE: "1", XDG_DATA_HOME: data },
  stdio: ["ignore", "pipe", "pipe"],
});
let stderr = "";
proc.stderr.on("data", (d) => (stderr += d));
const base = await new Promise((res, rej) => {
  const t = setTimeout(() => rej(new Error("engine tidak siap")), 20000);
  proc.stdout.on("data", (d) => { const m = String(d).match(/listening on (http:\/\/\S+)/); if (m) { clearTimeout(t); res(m[1]); } });
});
const q = `directory=${encodeURIComponent(ws)}`;
const api = async (method, path, body) => {
  const r = await fetch(base + path + (path.includes("?") ? "&" : "?") + q, {
    method, headers: { ...auth, ...(body ? { "Content-Type": "application/json" } : {}) }, body: body ? JSON.stringify(body) : undefined,
  });
  const t = await r.text();
  if (!r.ok) throw new Error(`${method} ${path} → ${r.status} ${t}`);
  return t ? JSON.parse(t) : null;
};

// --- rekam SSE ---
mkdirSync(out, { recursive: true });
const events = [];
const t0 = Date.now();
const listeners = new Set();
const ctl = new AbortController();
const sse = (async () => {
  const r = await fetch(base + "/event?" + q, { headers: auth, signal: ctl.signal });
  const dec = new TextDecoder();
  let buf = "";
  for await (const chunk of r.body) {
    buf += dec.decode(chunk, { stream: true });
    let i;
    while ((i = buf.indexOf("\n\n")) >= 0) {
      const block = buf.slice(0, i); buf = buf.slice(i + 2);
      const dataLines = block.split("\n").filter((l) => l.startsWith("data:")).map((l) => l.slice(5).trimStart());
      if (!dataLines.length) continue;
      const ev = JSON.parse(dataLines.join("\n"));
      events.push({ t: Date.now() - t0, ev });
      for (const l of listeners) l(ev);
    }
  }
})().catch((e) => { if (e.name !== "AbortError") console.error("SSE error", e); });
const waitFor = (pred, ms = 240000) => new Promise((res, rej) => {
  const l = (ev) => { if (pred(ev)) { listeners.delete(l); clearTimeout(t); res(ev); } };
  const t = setTimeout(() => { listeners.delete(l); rej(new Error("timeout menunggu event")); }, ms);
  listeners.add(l);
});
await new Promise((r) => setTimeout(r, 500));

// --- skenario ---
const session = await api("POST", "/session", { title: "Zandor S2 rekaman" });
const sid = session.id;
// Kebijakan izin otomatis: urutan balasan per skenario.
let permissionPolicy = "once";
const permissionLog = [];
listeners.add(async (ev) => {
  if (ev.type === "permission.asked" && ev.properties.sessionID === sid) {
    const reply = permissionPolicy;
    permissionLog.push({ permission: ev.properties.permission, patterns: ev.properties.patterns, reply });
    await api("POST", `/permission/${ev.properties.id}/reply`, reply === "reject" ? { reply, message: "Ditolak oleh skenario uji Zandor S2" } : { reply });
  }
});

const scenarios = [
  {
    name: "baca+lampiran+reasoning",
    policy: "once",
    parts: [
      { type: "text", text: "Pikirkan dulu langkahmu. Baca file add.js memakai tool read, lalu jelaskan singkat (2 kalimat) bug-nya dengan merujuk catatan terlampir. Jangan ubah apa pun." },
      { type: "file", mime: "text/plain", filename: "catatan.txt", url: pathToFileURL(join(ws, "catatan.txt")).href },
    ],
  },
  {
    name: "todo+edit+bash (izin once)",
    policy: "once",
    parts: [{ type: "text", text: "Buat todo list 2 langkah dengan tool todowrite, lalu perbaiki bug di add.js memakai tool edit (harus a + b), lalu jalankan `node add.js` dengan tool bash dan laporkan hasilnya. Tandai todo selesai." }],
  },
  {
    name: "bash ditolak (izin reject)",
    policy: "reject",
    parts: [{ type: "text", text: "Hapus file sampah.txt dengan tool bash (perintah rm atau del). Kalau ditolak, cukup jelaskan bahwa izin ditolak." }],
  },
];

const stats = [];
for (const s of scenarios) {
  permissionPolicy = s.policy;
  const startIdx = events.length;
  const idle = waitFor((ev) => ev.type === "session.idle" && ev.properties.sessionID === sid);
  const ts = Date.now();
  await api("POST", `/session/${sid}/prompt_async`, { model: { providerID, modelID }, parts: s.parts });
  await idle;
  stats.push({ scenario: s.name, ms: Date.now() - ts, events: events.length - startIdx });
  console.log(`selesai: ${s.name} (${Date.now() - ts} ms, ${events.length - startIdx} event)`);
}

await new Promise((r) => setTimeout(r, 800));
const messages = await api("GET", `/session/${sid}/message`);
const version = (await api("GET", "/global/health")).version;
ctl.abort();
await sse;

// Samarkan path pribadi (workspace, data, home, username) di setiap string sebelum disimpan.
const home = process.env.USERPROFILE || process.env.HOME || "";
const user = home.split(/[\\/]/).pop() || "";
const forms = (p) => {
  const fwd = p.replace(/\\/g, "/");
  const noDrive = p.replace(/^[A-Za-z]:/, "");
  return [p, fwd, noDrive, noDrive.replace(/\\/g, "/"), noDrive.replace(/^\\/, ""), encodeURI(fwd), encodeURI(noDrive.replace(/\\/g, "/"))];
};
const reps = [];
// Windows: TEMP bisa berupa nama pendek 8.3 (mis. NAMAPE~1) sementara opencode melaporkan
// path panjang → samarkan keduanya.
const long = (p) => { try { return realpathSync.native(p); } catch { return p; } };
for (const [p, tag] of [[ws, "<WS>"], [long(ws), "<WS>"], [data, "<DATA>"], [long(data), "<DATA>"], [home, "<HOME>"], [long(home), "<HOME>"], [tmpdir(), "<TMP>"], [long(tmpdir()), "<TMP>"]]) for (const f of forms(p)) if (f) reps.push([f, tag]);
reps.sort((a, b) => b[0].length - a[0].length);
const scrubStr = (s) => {
  for (const [f, tag] of reps) s = s.split(f).join(tag).split(f.toLowerCase()).join(tag);
  // Sapu bersih: path home dalam bentuk apa pun (termasuk JSON ber-escape di dalam teks,
  // nama pendek 8.3, URL-encoded) → <HOME>.
  s = s.replace(/([A-Za-z]:)?(\\+|\/)Users(\\+|\/)[^\\\/"'\s]+/gi, "<HOME>");
  s = s.replace(/([A-Za-z]:)?%5CUsers%5C[^%"'\s]+|\/Users\/[^\/"'\s]+/gi, "<HOME>");
  if (user) s = s.split(user).join("<USER>").split(encodeURIComponent(user)).join("<USER>");
  return s;
};
const deep = (v) => (typeof v === "string" ? scrubStr(v) : Array.isArray(v) ? v.map(deep) : v && typeof v === "object" ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, deep(x)])) : v);
const scrub = (txt) => {
  const leakParts = user ? [user, ...user.split(/\s+/).filter((w) => w.length > 3)] : [];
  for (const w of leakParts) {
    const i = txt.toLowerCase().indexOf(w.toLowerCase());
    if (i >= 0) {
      const ctx = txt.slice(Math.max(0, i - 60), i + 60);
      for (const x of leakParts) ctx.split(x).join("<X>");
      console.error("konteks bocor:", leakParts.reduce((c, x) => c.split(new RegExp(x, "ig")).join("<X>"), ctx));
      throw new Error("fixture masih memuat data pribadi; tidak ditulis");
    }
  }
  return txt;
};
writeFileSync(join(out, "events.ndjson"), scrub(events.map((e) => JSON.stringify(deep(e))).join("\n") + "\n"));
writeFileSync(join(out, "messages.json"), scrub(JSON.stringify(deep(messages), null, 2)));
const typeCounts = {};
for (const { ev } of events) typeCounts[ev.type] = (typeCounts[ev.type] || 0) + 1;
const partTypes = {};
for (const m of messages) for (const p of m.parts) partTypes[p.type] = (partTypes[p.type] || 0) + 1;
const toolNames = [...new Set(messages.flatMap((m) => m.parts.filter((p) => p.type === "tool").map((p) => `${p.tool}:${p.state.status}`)))];
writeFileSync(join(out, "meta.json"), scrub(JSON.stringify(deep({ opencode: version, model: `${providerID}/${modelID}`, recordedAt: new Date().toISOString(), workspacePlaceholder: ws, sessionID: sid, scenarios: stats, permissionLog, typeCounts, partTypes, toolNames }), null, 2)));
console.log(JSON.stringify({ typeCounts, partTypes, toolNames, permissionLog }, null, 2));

proc.kill();
await new Promise((r) => proc.once("exit", r));
for (const d of [ws, data]) { try { rmSync(d, { recursive: true, force: true }); } catch {} }
process.exit(0);
