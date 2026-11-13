// Daftar terminal per project (Fase 8): PTY dibuat di engine dengan `cwd` = folder project, sehingga terminal
// "mengikuti project aktif". Setelah aplikasi dimuat ulang, PTY yang masih hidup di engine diadopsi kembali
// (riwayat diputar ulang oleh engine saat WebSocket tersambung).
import { type PtyClient, type PtyInfo, pickShell, type ShellOption } from "./pty";

export type TerminalTab = {
  id: string;
  /** Kunci project (atau "chat") pemilik terminal. */
  key: string;
  cwd: string;
  title: string;
  status: "running" | "exited";
};

export type TerminalState = {
  tabs: TerminalTab[];
  activeByKey: Record<string, string>;
  shells: ShellOption[];
  /** Pesan galat terakhir (mis. gagal membuat terminal). */
  error: string | null;
};

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class TerminalStore {
  private snap: TerminalState = { tabs: [], activeByKey: {}, shells: [], error: null };
  private listeners = new Set<() => void>();
  private client: PtyClient | null = null;
  private counters = new Map<string, number>();

  state(): TerminalState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private update(patch: Partial<TerminalState>) {
    this.snap = { ...this.snap, ...patch };
    for (const l of this.listeners) l();
  }

  /** Engine siap/tidak: muat daftar shell. Terminal yang sudah ada dipertahankan (soket dipulihkan oleh tampilan). */
  setClient(client: PtyClient | null) {
    if (client === this.client) return;
    this.client = client;
    if (!client) return;
    void client.shells().then(
      (shells) => this.client === client && this.update({ shells }),
      (e) => this.client === client && this.update({ error: errMsg(e) }),
    );
  }

  tabsOf(key: string): TerminalTab[] {
    return this.snap.tabs.filter((t) => t.key === key);
  }

  /**
   * Adopsi PTY yang masih hidup di engine untuk `cwd` ini (setelah muat ulang). Mengembalikan jumlah yang diadopsi.
   * PTY yang sudah dikenal dilewati; yang sudah `exited` tidak diadopsi.
   */
  async adopt(key: string, cwd: string): Promise<number> {
    const client = this.client;
    if (!client) return 0;
    let list: PtyInfo[];
    try {
      list = await client.list(cwd);
    } catch {
      return 0;
    }
    if (client !== this.client) return 0;
    const fresh = list.filter((p) => p.status === "running" && !this.snap.tabs.some((t) => t.id === p.id));
    if (fresh.length === 0) return 0;
    const tabs = fresh.map<TerminalTab>((p) => ({ id: p.id, key, cwd, title: p.title, status: "running" }));
    const active = this.snap.activeByKey[key] ?? tabs[0]?.id;
    this.update({
      tabs: [...this.snap.tabs, ...tabs],
      activeByKey: active ? { ...this.snap.activeByKey, [key]: active } : this.snap.activeByKey,
    });
    return fresh.length;
  }

  async create(key: string, cwd: string, shellPath?: string): Promise<TerminalTab> {
    const client = this.client;
    if (!client) throw new Error("engine belum siap");
    const shell = shellPath ? this.snap.shells.find((s) => s.path === shellPath) : pickShell(this.snap.shells);
    if (!shell) throw new Error("tidak ada shell yang tersedia di engine");
    const n = (this.counters.get(key) ?? 0) + 1;
    this.counters.set(key, n);
    const isPwsh = /powershell|pwsh/i.test(shell.name);
    try {
      const p = await client.create(cwd, {
        command: shell.path,
        ...(isPwsh ? { args: ["-NoLogo"] } : {}),
        title: `${shell.name} ${n}`,
      });
      const tab: TerminalTab = { id: p.id, key, cwd, title: p.title, status: "running" };
      this.update({
        tabs: [...this.snap.tabs, tab],
        activeByKey: { ...this.snap.activeByKey, [key]: tab.id },
        error: null,
      });
      return tab;
    } catch (e) {
      this.update({ error: errMsg(e) });
      throw e;
    }
  }

  setActive(key: string, id: string) {
    if (this.snap.activeByKey[key] === id) return;
    this.update({ activeByKey: { ...this.snap.activeByKey, [key]: id } });
  }

  /** Tampilan memberi tahu bahwa proses shell berakhir (tab tetap ada sampai ditutup). */
  markExited(id: string) {
    if (!this.snap.tabs.some((t) => t.id === id && t.status !== "exited")) return;
    this.update({ tabs: this.snap.tabs.map((t) => (t.id === id ? { ...t, status: "exited" } : t)) });
  }

  async close(id: string): Promise<void> {
    const tab = this.snap.tabs.find((t) => t.id === id);
    if (!tab) return;
    const rest = this.snap.tabs.filter((t) => t.id !== id);
    const siblings = rest.filter((t) => t.key === tab.key);
    const active = { ...this.snap.activeByKey };
    if (active[tab.key] === id) {
      const next = siblings.at(-1);
      if (next) active[tab.key] = next.id;
      else delete active[tab.key];
    }
    this.update({ tabs: rest, activeByKey: active });
    try {
      await this.client?.remove(tab.cwd, id);
    } catch {
      // PTY mungkin sudah hilang di engine; tab sudah ditutup di UI
    }
  }
}
