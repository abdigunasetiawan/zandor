// Logika murni untuk menampilkan satu langkah tool seperti agent harness: berkas apa, baris berapa, perubahan (+/−), perintah
// dan kode keluarnya, hasil pencarian, sub-agen. Bentuk data mengikuti keluaran opencode 1.19.5 (diambil dari engine asli);
// semua field opsional karena tool lain/versi lain bisa berbeda, jadi setiap fungsi punya cadangan aman. Tanpa React.
import type { MessagePart, ToolPart } from "@zandor/core";

const str = (v: unknown): string => (typeof v === "string" ? v : "");
const num = (v: unknown): number | undefined => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
const rec = (v: unknown): Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : {};

export type ToolKind =
  | "read"
  | "edit"
  | "write"
  | "shell"
  | "search"
  | "web"
  | "task"
  | "skill"
  | "plan"
  | "ask"
  | "memory"
  | "generic";

export function toolKind(tool: string): ToolKind {
  const t = tool.toLowerCase();
  if (t === "read") return "read";
  if (t === "edit" || t === "multiedit" || t === "patch" || t === "apply_patch") return "edit";
  if (t === "write") return "write";
  if (t === "bash") return "shell";
  if (t === "grep" || t === "glob" || t === "list") return "search";
  if (t === "websearch" || t === "codesearch" || t === "webfetch") return "web";
  if (t === "task") return "task";
  if (t === "skill") return "skill";
  if (t === "todowrite" || t === "todoread") return "plan";
  if (t === "question") return "ask";
  if (t.includes("memory")) return "memory";
  return "generic";
}

export type SplitPath = { dir: string; name: string; full: string };

/** Pisahkan path jadi folder (hanya dua segmen terakhir, diawali "…/" bila lebih panjang) dan nama berkas. */
export function splitPath(path: string): SplitPath {
  const norm = path.replace(/\\/g, "/");
  const parts = norm.split("/").filter(Boolean);
  const name = parts.at(-1) ?? norm;
  const parents = parts.slice(0, -1);
  const shown = parents.slice(-2).join("/");
  const dir = shown ? `${parents.length > 2 ? "…/" : ""}${shown}/` : "";
  return { dir, name, full: path };
}

export type DiffRow = {
  kind: "hunk" | "add" | "del" | "ctx";
  text: string;
  /** Nomor baris di berkas lama/baru (kosong untuk header hunk dan baris tak bernomor). */
  oldNo?: number;
  newNo?: number;
};

/** Baris header yang dibuang dari diff unified (Index:, garis ====, --- dan +++ sebelum hunk pertama). */
const isDiffHeader = (l: string) =>
  l.startsWith("Index:") || /^={5,}$/.test(l) || l.startsWith("--- ") || l.startsWith("+++ ");

/**
 * Urai teks diff unified menjadi baris bernomor. Header sebelum `@@` dibuang; nomor baris dihitung dari header hunk
 * (`@@ -a,b +c,d @@`). Teks tanpa hunk (bukan unified) diurai per awalan tanpa nomor.
 */
export function parseDiff(text: string): DiffRow[] {
  const rows: DiffRow[] = [];
  let oldNo = 0;
  let newNo = 0;
  let inHunk = false;
  for (const raw of text.replace(/\r\n/g, "\n").split("\n")) {
    const hunk = raw.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (hunk) {
      inHunk = true;
      oldNo = Number(hunk[1]);
      newNo = Number(hunk[2]);
      rows.push({ kind: "hunk", text: raw });
      continue;
    }
    if (!inHunk && isDiffHeader(raw)) continue;
    if (raw === "" && rows.at(-1)?.text === "") continue;
    if (raw.startsWith("\\")) continue; // "\ No newline at end of file"
    if (raw.startsWith("+") && !(inHunk === false && raw.startsWith("+++"))) {
      rows.push({ kind: "add", text: raw, ...(inHunk ? { newNo: newNo++ } : {}) });
    } else if (raw.startsWith("-") && !(inHunk === false && raw.startsWith("---"))) {
      rows.push({ kind: "del", text: raw, ...(inHunk ? { oldNo: oldNo++ } : {}) });
    } else if (raw !== "") {
      rows.push({ kind: "ctx", text: raw, ...(inHunk ? { oldNo: oldNo++, newNo: newNo++ } : {}) });
    }
  }
  while (rows.at(-1)?.kind === "ctx" && rows.at(-1)?.text === "") rows.pop();
  return rows;
}

/** Jumlah baris tambah/hapus dari baris diff yang sudah diurai. */
export function diffStats(rows: DiffRow[]): { adds: number; dels: number } {
  let adds = 0;
  let dels = 0;
  for (const r of rows) {
    if (r.kind === "add") adds++;
    else if (r.kind === "del") dels++;
  }
  return { adds, dels };
}

/** Isi berkas baru sebagai baris "tambah" bernomor (tool write: tidak ada diff dari engine). */
export function contentRows(content: string): DiffRow[] {
  const lines = content.replace(/\r\n/g, "\n").split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines.map((l, i) => ({ kind: "add" as const, text: `+${l}`, newNo: i + 1 }));
}

export type NumberedLine = { no?: number; text: string };

/** Keluaran tool `read`: baris bernomor ("12: isi") tanpa pembungkus <path>/<content>; catatan penutup dipisah. */
export function readLines(output: string): { lines: NumberedLine[]; note: string } {
  const inner = output.match(/<content>\n?([\s\S]*?)\n?<\/content>/)?.[1] ?? output;
  const lines: NumberedLine[] = [];
  const notes: string[] = [];
  for (const l of inner.split("\n")) {
    const m = l.match(/^(\d+): ?(.*)$/);
    if (m) lines.push({ no: Number(m[1]), text: m[2] ?? "" });
    else if (l.startsWith("(") && l.endsWith(")")) notes.push(l.slice(1, -1));
    else if (l.trim() !== "" && !l.startsWith("<")) lines.push({ text: l });
  }
  return { lines, note: notes.join(" ") };
}

export type GrepGroup = { file: string; lines: NumberedLine[] };

/** Keluaran tool `grep` ("Found n matches" lalu "berkas:" diikuti "  Line n: teks") dikelompokkan per berkas. */
export function grepGroups(output: string): GrepGroup[] {
  const groups: GrepGroup[] = [];
  for (const l of output.split("\n")) {
    const hit = l.match(/^\s+Line (\d+): ?(.*)$/);
    if (hit) {
      groups.at(-1)?.lines.push({ no: Number(hit[1]), text: hit[2] ?? "" });
    } else if (l.endsWith(":") && !l.startsWith("Found ") && l.trim() !== "") {
      groups.push({ file: l.slice(0, -1), lines: [] });
    }
  }
  return groups.filter((g) => g.lines.length > 0);
}

/** Hasil sub-agen dari keluaran tool `task` (`<task_result>…</task_result>`). */
export function taskResult(output: string): string {
  return output.match(/<task_result>\n?([\s\S]*?)\n?<\/task_result>/)?.[1]?.trim() ?? output.trim();
}

export type TodoItem = { content: string; status: string };

export type ToolView = {
  kind: ToolKind;
  /** Target utama untuk ringkasan satu baris (path, perintah, kueri, URL, pola, deskripsi). */
  target: string;
  /** Bila target adalah berkas: dipisah folder + nama. */
  path?: SplitPath;
  /** Rentang baris yang dibaca ("120–220"); `total` = jumlah baris berkas bila diketahui. */
  range?: { from: number; to: number; total?: number };
  adds?: number;
  dels?: number;
  exit?: number;
  /** Jumlah hasil (grep/glob) atau item (todo). */
  count?: number;
  /** Jenis sub-agen (tool task). */
  agent?: string;
  /** Id sesi anak sub-agen. */
  childSession?: string;
  /** Teks diff unified dari engine (edit). */
  diff?: string;
  command?: string;
  description?: string;
  todos?: TodoItem[];
};

const lineCount = (s: string) => (s === "" ? 0 : s.replace(/\r\n/g, "\n").replace(/\n$/, "").split("\n").length);

/** Bangun tampilan satu tool part dari input + metadata + keluaran. */
export function describeTool(part: ToolPart): ToolView {
  const { state } = part;
  const input = state.input;
  const meta = rec(state.metadata);
  const kind = toolKind(part.tool);
  const filePath = str(input.filePath) || str(input.path) || str(meta.filepath);
  const base: ToolView = { kind, target: "" };

  if (kind === "read") {
    const display = rec(meta.display);
    const from = num(display.lineStart) ?? num(input.offset) ?? 1;
    const lim = num(input.limit);
    const to = num(display.lineEnd) ?? (lim ? from + lim - 1 : undefined);
    const total = num(display.totalLines);
    return {
      ...base,
      target: filePath,
      ...(filePath ? { path: splitPath(filePath) } : {}),
      ...(to !== undefined &&
      (input.offset !== undefined || input.limit !== undefined || display.lineStart !== undefined)
        ? { range: { from, to, ...(total !== undefined ? { total } : {}) } }
        : {}),
    };
  }
  if (kind === "edit" || kind === "write") {
    const filediff = rec(meta.filediff);
    const diff = str(meta.diff) || str(filediff.patch);
    let adds = num(filediff.additions);
    let dels = num(filediff.deletions);
    if (diff && (adds === undefined || dels === undefined)) {
      const s = diffStats(parseDiff(diff));
      adds = adds ?? s.adds;
      dels = dels ?? s.dels;
    }
    if (kind === "write" && adds === undefined) adds = lineCount(str(input.content));
    return {
      ...base,
      target: filePath,
      ...(filePath ? { path: splitPath(filePath) } : {}),
      ...(diff ? { diff } : {}),
      ...(adds !== undefined ? { adds } : {}),
      ...(dels !== undefined ? { dels } : {}),
    };
  }
  if (kind === "shell") {
    const command = str(input.command);
    const exit = num(meta.exit);
    return {
      ...base,
      target: command || state.title || "",
      command,
      ...(str(input.description) ? { description: str(input.description) } : {}),
      ...(exit !== undefined ? { exit } : {}),
    };
  }
  if (kind === "search") {
    const pattern = str(input.pattern);
    const count = num(meta.matches) ?? num(meta.count);
    return {
      ...base,
      target: pattern || filePath || state.title || "",
      ...(count !== undefined ? { count } : {}),
    };
  }
  if (kind === "web") {
    return { ...base, target: str(input.url) || str(input.query) || state.title || "" };
  }
  if (kind === "task") {
    const child = str(meta.sessionId);
    return {
      ...base,
      target: str(input.description) || state.title || "",
      ...(str(input.subagent_type) ? { agent: str(input.subagent_type) } : {}),
      ...(child ? { childSession: child } : {}),
    };
  }
  if (kind === "skill") return { ...base, target: str(input.name) || state.title || "" };
  if (kind === "plan") {
    const todos = (Array.isArray(input.todos) ? input.todos : [])
      .map((x) => ({ content: str(rec(x).content), status: str(rec(x).status) }))
      .filter((x) => x.content);
    return { ...base, target: "", ...(todos.length > 0 ? { todos, count: todos.length } : {}) };
  }
  const first = Object.values(input).find((v) => typeof v === "string") as string | undefined;
  return { ...base, target: state.title || first || "" };
}

/** Ringkasan per jenis untuk satu proses (mis. "12 dibaca · 3 berkas diedit · 8 perintah"). */
export type ProcessTally = {
  read: number;
  edit: number;
  write: number;
  shell: number;
  search: number;
  web: number;
  task: number;
  other: number;
  adds: number;
  dels: number;
};

export function tallyProcess(parts: MessagePart[]): ProcessTally {
  const t: ProcessTally = {
    read: 0,
    edit: 0,
    write: 0,
    shell: 0,
    search: 0,
    web: 0,
    task: 0,
    other: 0,
    adds: 0,
    dels: 0,
  };
  const editedFiles = new Set<string>();
  for (const p of parts) {
    if (p.type !== "tool") continue;
    const v = describeTool(p);
    if (v.kind === "read") t.read++;
    else if (v.kind === "edit" || v.kind === "write") {
      // berkas yang sama diedit berulang dihitung sekali; perubahan (+/−) tetap dijumlahkan
      const key = v.path?.full ?? p.id;
      if (!editedFiles.has(key)) {
        editedFiles.add(key);
        if (v.kind === "write") t.write++;
        else t.edit++;
      }
      t.adds += v.adds ?? 0;
      t.dels += v.dels ?? 0;
    } else if (v.kind === "shell") t.shell++;
    else if (v.kind === "search") t.search++;
    else if (v.kind === "web") t.web++;
    else if (v.kind === "task") t.task++;
    else t.other++;
  }
  return t;
}
