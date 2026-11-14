// Store Git Panel 4 (Fase 8). Host (Tauri) menjalankan `git` CLI di project terdaftar; store ini hanya menyimpan status,
// menjalankan aksi, dan memuat ulang. Semua aksi mengembalikan galat apa adanya dari git (mis. identitas belum diset).

export type FileChange = { path: string; origPath: string | null; index: string; worktree: string };
export type GitStatus = {
  isRepo: boolean;
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  detached: boolean;
  files: FileChange[];
};
export type Branch = { name: string; current: boolean };
export type CommitInfo = { hash: string; subject: string; author: string; date: string };

export interface GitHost {
  /** false di luar aplikasi desktop: tidak ada git. */
  available: boolean;
  status(root: string): Promise<GitStatus>;
  diff(root: string, path: string, staged: boolean, untracked: boolean): Promise<string>;
  stage(root: string, paths: string[]): Promise<void>;
  unstage(root: string, paths: string[]): Promise<void>;
  commit(root: string, message: string): Promise<string>;
  branches(root: string): Promise<Branch[]>;
  switchBranch(root: string, name: string): Promise<void>;
  createBranch(root: string, name: string): Promise<void>;
  log(root: string, count: number): Promise<CommitInfo[]>;
  /** Repo git di anak folder `root` (monorepo yang akarnya bukan repo). Opsional: tanpa ini hanya akar yang diperiksa. */
  repos?(root: string): Promise<string[]>;
}

export type DiffView = { path: string; staged: boolean; untracked: boolean; text: string | null; error: string | null };

export type GitState = {
  /** Folder project (akar terdaftar). */
  projectRoot: string | null;
  /** Repo aktif: akar project bila ia repo, atau salah satu repo di anak foldernya (monorepo). */
  root: string | null;
  /** Repo di anak folder project (terisi hanya bila akarnya bukan repo). */
  repos: string[];
  status: GitStatus | null;
  branches: Branch[];
  log: CommitInfo[];
  diff: DiffView | null;
  loading: boolean;
  /** Aksi (stage/commit/…) sedang berjalan. */
  busy: boolean;
  error: string | null;
};

const INITIAL: GitState = {
  projectRoot: null,
  root: null,
  repos: [],
  status: null,
  branches: [],
  log: [],
  diff: null,
  loading: false,
  busy: false,
  error: null,
};
const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));

export type ChangeGroups = { staged: FileChange[]; unstaged: FileChange[]; conflicts: FileChange[] };

/** Kelompokkan perubahan. Berkas bisa muncul di dua kelompok (staged dan unstaged sekaligus). */
export function groupChanges(files: FileChange[]): ChangeGroups {
  const out: ChangeGroups = { staged: [], unstaged: [], conflicts: [] };
  for (const f of files) {
    const conflict =
      f.index === "U" ||
      f.worktree === "U" ||
      (f.index === "A" && f.worktree === "A") ||
      (f.index === "D" && f.worktree === "D");
    if (conflict) {
      out.conflicts.push(f);
      continue;
    }
    if (f.index !== " " && f.index !== "?") out.staged.push(f);
    if (f.worktree !== " " || f.index === "?") out.unstaged.push(f);
  }
  return out;
}

export const isUntracked = (f: FileChange) => f.index === "?" && f.worktree === "?";

export class GitStore {
  private snap: GitState = INITIAL;
  private listeners = new Set<() => void>();
  private seq = 0;
  /** Repo yang dipilih pengguna per project (bertahan selama aplikasi berjalan). */
  private chosen = new Map<string, string>();

  constructor(private readonly host: GitHost) {}

  get available(): boolean {
    return this.host.available;
  }

  state(): GitState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private update(patch: Partial<GitState>) {
    this.snap = { ...this.snap, ...patch };
    for (const l of this.listeners) l();
  }

  /** Project aktif berubah → muat ulang. `null` (mis. folder Chat tanpa git) mengosongkan. */
  async setRoot(root: string | null): Promise<void> {
    if (root === this.snap.projectRoot) return;
    this.seq++;
    this.update({ ...INITIAL, projectRoot: root, root });
    if (root) await this.refresh();
  }

  /** Pilih repo di anak folder project (monorepo). */
  async selectRepo(dir: string): Promise<void> {
    const { projectRoot, repos } = this.snap;
    if (!projectRoot || !repos.includes(dir) || dir === this.snap.root) return;
    this.chosen.set(projectRoot, dir);
    this.seq++;
    this.update({ root: dir, status: null, branches: [], log: [], diff: null, error: null });
    await this.refresh();
  }

  async refresh(): Promise<void> {
    const root = this.snap.root;
    if (!root || !this.host.available) return;
    const n = ++this.seq;
    this.update({ loading: true, error: null });
    try {
      let status = await this.host.status(root);
      let activeRoot = root;
      let repos = this.snap.repos;
      // Monorepo: akar project bukan repo, tetapi anak folder mungkin repo → cari dan pakai salah satunya.
      if (!status.isRepo && root === this.snap.projectRoot && this.host.repos) {
        repos = await this.host.repos(root).catch(() => []);
        const pick = repos.find((r) => r === this.chosen.get(root)) ?? repos[0];
        if (pick) {
          activeRoot = pick;
          status = await this.host.status(pick);
        }
      }
      const [branches, log] = status.isRepo
        ? await Promise.all([
            this.host.branches(activeRoot).catch(() => []),
            this.host.log(activeRoot, 15).catch(() => []),
          ])
        : [[], []];
      if (n !== this.seq || (root !== this.snap.root && root !== this.snap.projectRoot)) return;
      // diff yang dibuka tetap ditampilkan hanya bila berkasnya masih berubah
      const d = this.snap.diff;
      const stillThere = d && status.files.some((f) => f.path === d.path);
      this.update({ root: activeRoot, repos, status, branches, log, loading: false, diff: stillThere ? d : null });
    } catch (e) {
      if (n !== this.seq) return;
      this.update({ loading: false, error: errMsg(e) });
    }
  }

  private async act(fn: (root: string) => Promise<unknown>): Promise<void> {
    const root = this.snap.root;
    if (!root) throw new Error("tidak ada project aktif");
    this.update({ busy: true, error: null });
    try {
      await fn(root);
    } catch (e) {
      this.update({ busy: false, error: errMsg(e) });
      throw e;
    }
    this.update({ busy: false });
    await this.refresh();
  }

  stage(paths: string[]) {
    return this.act((r) => this.host.stage(r, paths));
  }

  unstage(paths: string[]) {
    return this.act((r) => this.host.unstage(r, paths));
  }

  /** Commit semua yang sudah di-stage. Mengembalikan hash pendek. */
  async commit(message: string): Promise<string> {
    let hash = "";
    await this.act(async (r) => {
      hash = await this.host.commit(r, message);
    });
    return hash;
  }

  switchBranch(name: string) {
    return this.act((r) => this.host.switchBranch(r, name));
  }

  createBranch(name: string) {
    return this.act((r) => this.host.createBranch(r, name));
  }

  async showDiff(file: FileChange, staged: boolean): Promise<void> {
    const root = this.snap.root;
    if (!root) return;
    const untracked = isUntracked(file);
    const view: DiffView = { path: file.path, staged, untracked, text: null, error: null };
    this.update({ diff: view });
    try {
      const text = await this.host.diff(root, file.path, staged, untracked);
      if (this.snap.diff?.path === file.path && this.snap.diff.staged === staged)
        this.update({ diff: { ...view, text } });
    } catch (e) {
      if (this.snap.diff?.path === file.path) this.update({ diff: { ...view, error: errMsg(e) } });
    }
  }

  closeDiff() {
    if (this.snap.diff) this.update({ diff: null });
  }
}
