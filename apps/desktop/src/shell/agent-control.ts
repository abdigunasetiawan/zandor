// Kontrol aplikasi oleh agent (ADR-077): menjalankan permintaan dari tool `zandor_app`/`zandor_image` di plugin engine.
// Permintaan datang dari model (tidak dipercaya): setiap aksi divalidasi di sini; hanya aksi tampilan/aplikasi yang
// tersedia (tidak ada akses berkas/shell baru — itu tetap lewat tool engine yang meminta izin).
import { SETTING_KEYS, type SettingKey, validateSetting } from "@zandor/core";
import { PANEL_IDS, type PanelId, TOOL_IDS, type ToolId } from "./services";

export type ControlRequest = {
  id: string;
  action: string;
  args: Record<string, unknown>;
  sessionID: string;
  directory: string;
};

export type ControlResponse = { ok: true; result?: unknown } | { ok: false; error: string };

/** Pengaturan yang tidak boleh diubah agent: izin kontrol agent sendiri. */
export const PROTECTED_SETTINGS: readonly SettingKey[] = ["agentControl"];
/** Perintah palet yang tidak boleh dijalankan agent (menutup aplikasi, alat pengembang). */
export const BLOCKED_COMMANDS: readonly string[] = ["window.close", "dev.openDevtools", "dev.copyChatHtml"];

export type ImageSize = "1024x1024" | "1536x1024" | "1024x1536";
export const IMAGE_SIZES: readonly ImageSize[] = ["1024x1024", "1536x1024", "1024x1536"];

export interface ControlDeps {
  enabled(): boolean;
  status(): unknown;
  listCommands(): { id: string; title: string; enabled: boolean }[];
  runCommand(id: string): Promise<boolean>;
  panel(panel: PanelId, state: "open" | "close" | "toggle"): void;
  openTool(tool: ToolId): void;
  openUrl(url: string): void;
  /** false bila berkas di luar project/Chat. */
  openFile(path: string, root: string): boolean;
  isRoute(path: string): boolean;
  navigate(path: string): void;
  listTerminals(): { id: string; title: string; cwd: string; status: string }[];
  closeTerminal(id: string): Promise<boolean>;
  getSettings(): Record<string, unknown>;
  setSetting(key: SettingKey, value: unknown): void;
  /** Mengembalikan path relatif berkas yang ditulis. */
  generateImage(prompt: string, size: ImageSize, root: string, rel: string): Promise<string>;
  /** Folder project yang dikenal aplikasi (untuk memvalidasi `directory` permintaan). */
  knownRoots(): string[];
}

const str = (v: unknown) => (typeof v === "string" ? v.trim() : "");
const fail = (error: string): ControlResponse => ({ ok: false, error });
const sameDir = (a: string, b: string) =>
  a
    .replace(/[\\/]+$/, "")
    .replace(/\\/g, "/")
    .toLowerCase() ===
  b
    .replace(/[\\/]+$/, "")
    .replace(/\\/g, "/")
    .toLowerCase();

/** Nama berkas aman dari prompt: huruf kecil, kebab, maks 40 karakter. */
export function imageFileName(prompt: string, now = new Date()): string {
  const slug =
    prompt
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 40)
      .replace(/-+$/, "") || "gambar";
  const p = (n: number) => String(n).padStart(2, "0");
  const stamp = `${now.getFullYear()}${p(now.getMonth() + 1)}${p(now.getDate())}-${p(now.getHours())}${p(now.getMinutes())}${p(now.getSeconds())}`;
  return `images/${slug}-${stamp}.png`;
}

/** Path relatif yang aman di dalam folder project (tanpa `..`, tanpa absolut). */
export function safeRelPath(path: string): string | null {
  const p = path.trim().replace(/\\/g, "/").replace(/^\.\//, "");
  if (!p || p.startsWith("/") || /^[a-z]:/i.test(p)) return null;
  if (p.split("/").some((s) => s === ".." || s === "")) return null;
  return p;
}

export async function executeControl(req: ControlRequest, deps: ControlDeps): Promise<ControlResponse> {
  if (!deps.enabled())
    return fail(
      "Kontrol aplikasi oleh agent dimatikan pengguna (Pengaturan → Umum → Agent boleh mengoperasikan Zandor).",
    );
  const a = req.args ?? {};
  try {
    switch (req.action) {
      case "status":
        return { ok: true, result: deps.status() };
      case "list_commands":
        return { ok: true, result: deps.listCommands().filter((c) => !BLOCKED_COMMANDS.includes(c.id)) };
      case "run_command": {
        const id = str(a.id);
        if (!id) return fail("id perintah wajib (lihat list_commands).");
        if (BLOCKED_COMMANDS.includes(id)) return fail(`Perintah "${id}" tidak boleh dijalankan agent.`);
        return (await deps.runCommand(id))
          ? { ok: true, result: `Perintah "${id}" dijalankan.` }
          : fail(`Perintah "${id}" tidak ada atau sedang nonaktif.`);
      }
      case "panel": {
        const panel = str(a.panel) as PanelId;
        const state = (str(a.state) || "toggle") as "open" | "close" | "toggle";
        if (!PANEL_IDS.includes(panel)) return fail(`panel harus salah satu: ${PANEL_IDS.join(", ")}`);
        if (!["open", "close", "toggle"].includes(state)) return fail("state harus open, close, atau toggle");
        deps.panel(panel, state);
        return { ok: true, result: `Panel ${panel}: ${state}.` };
      }
      case "open_tool": {
        const tool = str(a.tool) as ToolId;
        if (!TOOL_IDS.includes(tool)) return fail(`tool harus salah satu: ${TOOL_IDS.join(", ")}`);
        deps.openTool(tool);
        return { ok: true, result: `Alat ${tool} dibuka di Panel 4.` };
      }
      case "open_url": {
        const url = str(a.url);
        let parsed: URL;
        try {
          parsed = new URL(url);
        } catch {
          return fail("url tidak valid.");
        }
        if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return fail("Hanya URL http/https.");
        if (parsed.username || parsed.password) return fail("URL dengan kredensial ditolak.");
        deps.openUrl(parsed.toString());
        return { ok: true, result: `Membuka ${parsed.toString()} di browser Panel 4.` };
      }
      case "open_file": {
        const path = str(a.path);
        if (!path) return fail("path wajib.");
        return deps.openFile(path, req.directory)
          ? { ok: true, result: `${path} dibuka di Panel 3.` }
          : fail(`${path} berada di luar folder project/Chat yang dikenal Zandor.`);
      }
      case "navigate": {
        const page = str(a.page).replace(/^\/+/, "");
        const path = page === "home" || page === "" ? "/" : `/${page}`;
        if (!deps.isRoute(path)) return fail(`Halaman "${page}" tidak dikenal.`);
        deps.navigate(path);
        return { ok: true, result: `Membuka ${path}.` };
      }
      case "list_terminals":
        return { ok: true, result: deps.listTerminals() };
      case "close_terminal": {
        const id = str(a.id);
        if (!id) return fail("id terminal wajib (lihat list_terminals).");
        return (await deps.closeTerminal(id))
          ? { ok: true, result: `Terminal ${id} ditutup.` }
          : fail(`Terminal "${id}" tidak ditemukan.`);
      }
      case "get_settings":
        return { ok: true, result: deps.getSettings() };
      case "set_setting": {
        const key = str(a.key) as SettingKey;
        if (!SETTING_KEYS.includes(key)) return fail(`Kunci pengaturan "${key}" tidak dikenal (lihat get_settings).`);
        if (PROTECTED_SETTINGS.includes(key)) return fail(`Pengaturan "${key}" hanya bisa diubah pengguna.`);
        const err = validateSetting(key, a.value);
        if (err) return fail(err);
        deps.setSetting(key, a.value);
        return { ok: true, result: `${key} = ${JSON.stringify(a.value)}` };
      }
      case "generate_image": {
        const prompt = str(a.prompt);
        if (!prompt) return fail("prompt wajib.");
        const size = (str(a.size) || "1024x1024") as ImageSize;
        if (!IMAGE_SIZES.includes(size)) return fail(`size harus salah satu: ${IMAGE_SIZES.join(", ")}`);
        const root = deps.knownRoots().find((r) => sameDir(r, req.directory));
        if (!root) return fail("Folder sesi ini tidak dikenal Zandor; gambar tidak disimpan.");
        const rel = a.path === undefined ? imageFileName(prompt) : safeRelPath(str(a.path));
        if (!rel) return fail("path harus relatif di dalam folder project (tanpa ..).");
        const written = await deps.generateImage(prompt, size, root, rel);
        deps.openFile(written, root);
        return { ok: true, result: `Gambar disimpan: ${written} (dibuka di Panel 3).` };
      }
      default:
        return fail(`Aksi "${req.action}" tidak dikenal.`);
    }
  } catch (e) {
    return fail(e instanceof Error ? e.message : String(e));
  }
}
