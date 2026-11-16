// Klien API `/pty` milik engine (ADR-024): daftar shell, buat/ubah ukuran/hapus PTY, dan WebSocket I/O dengan tiket
// sekali pakai (WebSocket browser tidak bisa mengirim header Authorization). Terminal berjalan di mesin engine,
// sehingga otomatis benar untuk engine remote nanti (Fase 11).
import { basicAuthHeader, type EngineConnectionInfo } from "../engine/api";

export type ShellOption = {
  path: string;
  name: string;
  acceptable: boolean;
  /** Jenis stabil (pwsh, powershell, cmd, git-bash, wsl, bash, …) dari deteksi aplikasi desktop; untuk ikon cadangan. */
  kind?: string;
  /** Argumen peluncuran (mis. `-d Ubuntu` untuk distro WSL). Tanpa ini dipakai bawaan per jenis shell. */
  args?: string[];
  /** Ikon asli aplikasi, PNG base64 tanpa awalan `data:`. */
  icon?: string;
};

/** Shell yang dideteksi aplikasi desktop di mesin pengguna (perintah Tauri `terminal_shells`). */
export type DetectedShell = { id: string; name: string; path: string; args?: string[]; icon?: string };

const shellKey = (path: string, args: readonly string[] = []) => [path.toLowerCase(), ...args].join("\u0000");

/**
 * Gabungkan daftar engine (`/pty/shells`, sering tidak lengkap) dengan hasil deteksi lokal. Urutan deteksi dipertahankan
 * (sudah dikurasi: PowerShell, cmd, Git Bash, WSL per distro, …), lalu shell yang HANYA dikenal engine. Entri engine
 * dibuang bila exe yang sama sudah ada di deteksi; penanda `acceptable: false` dari engine tetap dihormati.
 */
export function mergeShells(engine: ShellOption[], detected: DetectedShell[]): ShellOption[] {
  const rejected = new Set(engine.filter((s) => !s.acceptable).map((s) => s.path.toLowerCase()));
  const seen = new Set<string>();
  const out: ShellOption[] = [];
  for (const d of detected) {
    const key = shellKey(d.path, d.args);
    if (!d.path || seen.has(key)) continue;
    seen.add(key);
    out.push({
      path: d.path,
      name: d.name || d.path,
      kind: d.id,
      acceptable: !rejected.has(d.path.toLowerCase()),
      ...(d.args?.length ? { args: [...d.args] } : {}),
      ...(d.icon ? { icon: d.icon } : {}),
    });
  }
  const detectedPaths = new Set(detected.map((d) => d.path.toLowerCase()));
  for (const s of engine) {
    if (detectedPaths.has(s.path.toLowerCase()) || seen.has(shellKey(s.path))) continue;
    seen.add(shellKey(s.path));
    out.push(s);
  }
  return out;
}

/** Kunci unik satu pilihan shell (exe yang sama bisa muncul beberapa kali dengan argumen berbeda, mis. distro WSL). */
export function shellId(s: Pick<ShellOption, "path" | "args">): string {
  return s.args?.length ? `${s.path}\u0000${s.args.join("\u0000")}` : s.path;
}
export type PtyInfo = {
  id: string;
  title: string;
  command: string;
  cwd: string;
  status: "running" | "exited";
  pid: number | null;
};

export interface PtySocket {
  send(data: string): void;
  /** Output terminal (teks atau byte). */
  onData(cb: (chunk: string | Uint8Array) => void): void;
  /** Posisi replay terbaru dari server (untuk reconnect tanpa kehilangan/menggandakan riwayat). */
  onCursor(cb: (cursor: number) => void): void;
  onClose(cb: () => void): void;
  close(): void;
}

export interface PtyClient {
  shells(): Promise<ShellOption[]>;
  list(directory: string): Promise<PtyInfo[]>;
  create(directory: string, input: { command: string; args?: string[]; title?: string }): Promise<PtyInfo>;
  resize(directory: string, id: string, cols: number, rows: number): Promise<void>;
  remove(directory: string, id: string): Promise<void>;
  connect(directory: string, id: string, cursor?: number): Promise<PtySocket>;
}

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);

export function toPtyInfo(raw: unknown): PtyInfo | null {
  if (!isObj(raw) || typeof raw.id !== "string") return null;
  return {
    id: raw.id,
    title: typeof raw.title === "string" ? raw.title : "terminal",
    command: typeof raw.command === "string" ? raw.command : "",
    cwd: typeof raw.cwd === "string" ? raw.cwd : "",
    status: raw.status === "exited" ? "exited" : "running",
    pid: typeof raw.pid === "number" ? raw.pid : null,
  };
}

export function toShell(raw: unknown): ShellOption | null {
  if (!isObj(raw) || typeof raw.path !== "string") return null;
  return {
    path: raw.path,
    name: typeof raw.name === "string" ? raw.name : raw.path,
    acceptable: raw.acceptable !== false,
  };
}

/** Pilih shell bawaan: PowerShell bila ada (perilaku paling konsisten di Windows), lalu yang pertama yang layak. */
export function pickShell(shells: ShellOption[]): ShellOption | null {
  const ok = shells.filter((s) => s.acceptable);
  return (
    ok.find((s) => s.kind === "powershell" || /^powershell$/i.test(s.name)) ??
    ok.find((s) => s.kind === "pwsh" || /^(pwsh|bash|zsh|sh)$/i.test(s.name)) ??
    ok[0] ??
    null
  );
}

export type WebSocketCtor = new (url: string) => WebSocket;

export function createPtyClient(
  info: EngineConnectionInfo,
  fetchImpl: typeof fetch = (...a) => fetch(...a),
  WS: WebSocketCtor = WebSocket,
): PtyClient {
  const auth = basicAuthHeader(info);
  const base = info.url.replace(/\/+$/, "");
  const call = async (
    method: string,
    path: string,
    directory: string | null,
    body?: unknown,
    headers: Record<string, string> = {},
  ) => {
    const q = directory ? `${path.includes("?") ? "&" : "?"}directory=${encodeURIComponent(directory)}` : "";
    const r = await fetchImpl(`${base}${path}${q}`, {
      method,
      headers: {
        authorization: auth,
        ...(body !== undefined ? { "content-type": "application/json" } : {}),
        ...headers,
      },
      ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
    });
    const text = await r.text();
    if (!r.ok) throw new Error(`${method} ${path} → HTTP ${r.status}${text ? `: ${text.slice(0, 200)}` : ""}`);
    return text ? (JSON.parse(text) as unknown) : null;
  };
  return {
    async shells() {
      const v = await call("GET", "/pty/shells", null);
      return (Array.isArray(v) ? v : []).map(toShell).filter((s): s is ShellOption => s !== null);
    },
    async list(directory) {
      const v = await call("GET", "/pty", directory);
      return (Array.isArray(v) ? v : []).map(toPtyInfo).filter((p): p is PtyInfo => p !== null);
    },
    async create(directory, input) {
      const v = await call("POST", "/pty", directory, { ...input, cwd: directory });
      const p = toPtyInfo(v);
      if (!p) throw new Error("respons /pty tidak dikenali");
      return p;
    },
    async resize(directory, id, cols, rows) {
      await call("PUT", `/pty/${encodeURIComponent(id)}`, directory, { size: { cols, rows } });
    },
    async remove(directory, id) {
      await call("DELETE", `/pty/${encodeURIComponent(id)}`, directory);
    },
    async connect(directory, id, cursor = 0) {
      const tok = (await call("POST", `/pty/${encodeURIComponent(id)}/connect-token`, directory, undefined, {
        "x-opencode-ticket": "1",
      })) as { ticket?: string } | null;
      if (!tok?.ticket) throw new Error("engine tidak memberi tiket terminal");
      const url = `${base.replace(/^http/, "ws")}/pty/${encodeURIComponent(id)}/connect?ticket=${encodeURIComponent(tok.ticket)}&cursor=${cursor}&directory=${encodeURIComponent(directory)}`;
      const ws = new WS(url);
      ws.binaryType = "arraybuffer";
      const dataCbs: Array<(c: string | Uint8Array) => void> = [];
      const cursorCbs: Array<(n: number) => void> = [];
      const closeCbs: Array<() => void> = [];
      // Engine memutar ulang riwayat segera setelah tersambung: tampung sampai ada penerima, agar tidak hilang.
      const pendingData: Array<string | Uint8Array> = [];
      const emitData = (c: string | Uint8Array) => {
        if (dataCbs.length === 0) pendingData.push(c);
        else for (const cb of dataCbs) cb(c);
      };
      ws.onmessage = (m) => {
        if (typeof m.data === "string") {
          emitData(m.data);
          return;
        }
        const u = new Uint8Array(m.data as ArrayBuffer);
        if (u[0] === 0) {
          try {
            const c = (JSON.parse(new TextDecoder().decode(u.subarray(1))) as { cursor?: number }).cursor;
            if (typeof c === "number") for (const cb of cursorCbs) cb(c);
          } catch {
            // bingkai kursor rusak: diabaikan
          }
          return;
        }
        emitData(u);
      };
      ws.onclose = () => {
        for (const cb of closeCbs) cb();
      };
      await new Promise<void>((resolve, reject) => {
        ws.onopen = () => resolve();
        ws.onerror = () => reject(new Error("koneksi terminal gagal"));
      });
      return {
        send: (d) => {
          if (ws.readyState === 1) ws.send(d);
        },
        onData: (cb) => {
          dataCbs.push(cb);
          for (const c of pendingData.splice(0)) cb(c);
        },
        onCursor: (cb) => void cursorCbs.push(cb),
        onClose: (cb) => void closeCbs.push(cb),
        close: () => ws.close(),
      };
    },
  };
}
