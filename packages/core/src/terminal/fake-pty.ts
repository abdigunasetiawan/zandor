// PTY palsu di memori untuk test (core dan UI): meniru kontrak /pty engine (daftar shell, buat, resize, hapus, soket).
import type { PtyClient, PtyInfo, PtySocket, ShellOption } from "./pty";

export type FakePtySocket = PtySocket & {
  written: string[];
  closed: boolean;
  /** Kirim output seolah dari shell. */
  emit(chunk: string | Uint8Array): void;
  /** Simulasikan soket terputus. */
  drop(): void;
};

export type FakePty = PtyClient & {
  shellList: ShellOption[];
  ptys: Map<string, PtyInfo>;
  created: Array<{ directory: string; command: string; args?: string[]; title?: string }>;
  resizes: Array<{ id: string; cols: number; rows: number }>;
  removed: string[];
  sockets: FakePtySocket[];
  connects: Array<{ id: string; cursor: number }>;
  failCreate: string | null;
};

export function createFakePty(): FakePty {
  let n = 0;
  const f: FakePty = {
    shellList: [
      { path: "C:\\Windows\\powershell.exe", name: "powershell", acceptable: true },
      { path: "C:\\Windows\\cmd.exe", name: "cmd", acceptable: true },
    ],
    ptys: new Map(),
    created: [],
    resizes: [],
    removed: [],
    sockets: [],
    connects: [],
    failCreate: null,
    async shells() {
      return [...f.shellList];
    },
    async list(directory) {
      return [...f.ptys.values()].filter((p) => p.cwd === directory);
    },
    async create(directory, input) {
      if (f.failCreate) throw new Error(f.failCreate);
      f.created.push({ directory, ...input });
      const info: PtyInfo = {
        id: `pty_${++n}`,
        title: input.title ?? "terminal",
        command: input.command,
        cwd: directory,
        status: "running",
        pid: 1000 + n,
      };
      f.ptys.set(info.id, info);
      return info;
    },
    async resize(_d, id, cols, rows) {
      f.resizes.push({ id, cols, rows });
    },
    async remove(_d, id) {
      f.removed.push(id);
      f.ptys.delete(id);
    },
    async connect(_d, id, cursor = 0) {
      f.connects.push({ id, cursor });
      const data: Array<(c: string | Uint8Array) => void> = [];
      const closes: Array<() => void> = [];
      const sock: FakePtySocket = {
        written: [],
        closed: false,
        send: (d) => void sock.written.push(d),
        onData: (cb) => void data.push(cb),
        onCursor: () => {},
        onClose: (cb) => void closes.push(cb),
        close: () => {
          sock.closed = true;
        },
        emit: (c) => {
          for (const cb of data) cb(c);
        },
        drop: () => {
          for (const cb of closes) cb();
        },
      };
      f.sockets.push(sock);
      return sock;
    },
  };
  return f;
}
