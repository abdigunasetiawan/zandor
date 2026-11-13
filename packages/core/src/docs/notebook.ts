// Pembaca Jupyter Notebook (.ipynb) untuk penampil dokumen (Fase 11). Hanya membaca JSON; output HTML/JS TIDAK dirender
// (hanya gambar png/jpeg, teks, dan galat) sehingga tidak ada kode dari notebook yang berjalan.
const MAX_CELLS = 1000;
const MAX_OUTPUT_CHARS = 20_000;
const MAX_IMAGE_B64 = 3_000_000;

export type NotebookOutput =
  | { kind: "text"; text: string; stream?: "stdout" | "stderr" }
  | { kind: "image"; src: string }
  | { kind: "error"; name: string; message: string; traceback: string }
  | { kind: "skipped"; reason: string };

export type NotebookCell = {
  type: "markdown" | "code" | "raw";
  source: string;
  executionCount: number | null;
  outputs: NotebookOutput[];
};
export type Notebook = { language: string; cells: NotebookCell[]; truncated: boolean };

const join = (v: unknown): string => (Array.isArray(v) ? v.map(String).join("") : typeof v === "string" ? v : "");
const clip = (s: string) => (s.length > MAX_OUTPUT_CHARS ? `${s.slice(0, MAX_OUTPUT_CHARS)}\n…(dipotong)` : s);
// biome-ignore lint/suspicious/noControlCharactersInRegex: membuang urutan ANSI (traceback berwarna)
const ANSI = /\u001b\[[0-9;?]*[ -/]*[@-~]/g;

function outputOf(o: Record<string, unknown>): NotebookOutput | null {
  const t = o.output_type;
  if (t === "stream")
    return { kind: "text", text: clip(join(o.text)), stream: o.name === "stderr" ? "stderr" : "stdout" };
  if (t === "error") {
    return {
      kind: "error",
      name: String(o.ename ?? "Error"),
      message: String(o.evalue ?? ""),
      traceback: clip(join(o.traceback).replace(ANSI, "")),
    };
  }
  if (t === "execute_result" || t === "display_data") {
    const data = (o.data ?? {}) as Record<string, unknown>;
    for (const mime of ["image/png", "image/jpeg"] as const) {
      const b64 = join(data[mime]).replace(/\s+/g, "");
      if (b64) {
        if (b64.length > MAX_IMAGE_B64 || !/^[A-Za-z0-9+/=]+$/.test(b64))
          return { kind: "skipped", reason: "gambar terlalu besar atau tidak valid" };
        return { kind: "image", src: `data:${mime};base64,${b64}` };
      }
    }
    const plain = join(data["text/plain"]);
    if (plain) return { kind: "text", text: clip(plain) };
    if (data["text/html"] || data["application/javascript"] || data["application/vnd.jupyter.widget-view+json"])
      return { kind: "skipped", reason: "output interaktif/HTML tidak ditampilkan" };
    return null;
  }
  return null;
}

export function parseNotebook(text: string): Notebook {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    throw new Error("bukan notebook yang valid (JSON rusak)");
  }
  const nb = raw as {
    cells?: unknown;
    metadata?: { language_info?: { name?: string }; kernelspec?: { language?: string } };
  };
  if (!nb || typeof nb !== "object" || !Array.isArray(nb.cells))
    throw new Error("bukan notebook yang valid (tidak ada `cells`)");
  const cells: NotebookCell[] = [];
  for (const c of nb.cells.slice(0, MAX_CELLS) as Array<Record<string, unknown>>) {
    const type = c.cell_type === "markdown" ? "markdown" : c.cell_type === "code" ? "code" : "raw";
    const outputs: NotebookOutput[] = [];
    if (type === "code" && Array.isArray(c.outputs))
      for (const o of c.outputs as Array<Record<string, unknown>>) {
        const out = outputOf(o);
        if (out) outputs.push(out);
      }
    cells.push({
      type,
      source: join(c.source),
      executionCount: typeof c.execution_count === "number" ? c.execution_count : null,
      outputs,
    });
  }
  return {
    language: nb.metadata?.language_info?.name ?? nb.metadata?.kernelspec?.language ?? "python",
    cells,
    truncated: nb.cells.length > MAX_CELLS,
  };
}
