// Fungsi murni untuk aksi render kaya (salin/simpan per jenis konten): konversi tabel ke Markdown/TSV/CSV, ekstensi berkas
// dari bahasa blok kode, dan nama berkas bercap waktu. Dipisah dari komponen agar mudah diuji.

/** Sel tabel sebagai matriks teks (baris kepala ikut sebagai baris pertama). */
export function tableMatrix(table: HTMLTableElement): string[][] {
  return Array.from(table.rows).map((r) =>
    Array.from(r.cells).map((c) => (c.textContent ?? "").replace(/\s+/g, " ").trim()),
  );
}

export function toMarkdownTable(rows: string[][]): string {
  if (rows.length === 0) return "";
  const width = Math.max(...rows.map((r) => r.length));
  const pad = (r: string[]) => Array.from({ length: width }, (_, i) => (r[i] ?? "").replace(/\|/g, "\\|"));
  const line = (r: string[]) => `| ${pad(r).join(" | ")} |`;
  const [head, ...body] = rows;
  return [line(head ?? []), `| ${Array.from({ length: width }, () => "---").join(" | ")} |`, ...body.map(line)].join(
    "\n",
  );
}

/** TSV: ditempel ke Excel/Google Sheets menjadi sel terpisah. */
export const toTsv = (rows: string[][]): string =>
  rows.map((r) => r.map((c) => c.replace(/\t/g, " ")).join("\t")).join("\n");

export function toCsv(rows: string[][]): string {
  const cell = (c: string) => (/[",\n\r]/.test(c) ? `"${c.replace(/"/g, '""')}"` : c);
  return rows.map((r) => r.map(cell).join(",")).join("\r\n");
}

const EXT: Record<string, string> = {
  javascript: "js",
  js: "js",
  typescript: "ts",
  ts: "ts",
  tsx: "tsx",
  jsx: "jsx",
  python: "py",
  py: "py",
  rust: "rs",
  rs: "rs",
  go: "go",
  java: "java",
  kotlin: "kt",
  c: "c",
  cpp: "cpp",
  "c++": "cpp",
  csharp: "cs",
  cs: "cs",
  php: "php",
  ruby: "rb",
  rb: "rb",
  swift: "swift",
  bash: "sh",
  sh: "sh",
  shell: "sh",
  zsh: "sh",
  powershell: "ps1",
  ps1: "ps1",
  html: "html",
  xml: "xml",
  css: "css",
  scss: "scss",
  json: "json",
  yaml: "yaml",
  yml: "yml",
  toml: "toml",
  sql: "sql",
  markdown: "md",
  md: "md",
  dockerfile: "Dockerfile",
  latex: "tex",
  tex: "tex",
  mermaid: "mmd",
};

/** Ekstensi berkas untuk bahasa blok kode; bahasa tak dikenal → `txt`. */
export const extForLang = (lang: string | null): string => (lang ? (EXT[lang.toLowerCase()] ?? "txt") : "txt");

/** `dasar-YYYYMMDD-HHMMSS.ext` (Dockerfile tanpa titik ekstensi). */
export function stampedName(base: string, ext: string, at: Date = new Date()): string {
  if (ext === "Dockerfile") return "Dockerfile";
  const p = (n: number) => String(n).padStart(2, "0");
  const stamp = `${at.getFullYear()}${p(at.getMonth() + 1)}${p(at.getDate())}-${p(at.getHours())}${p(at.getMinutes())}${p(at.getSeconds())}`;
  return `${base}-${stamp}.${ext}`;
}

/** Sumber LaTeX dari keluaran KaTeX (anotasi `application/x-tex`). */
type Hast = {
  type?: string;
  tagName?: string;
  value?: string;
  properties?: Record<string, unknown>;
  children?: Hast[];
};
export function latexOf(node: Hast | undefined): string | null {
  if (!node) return null;
  if (node.tagName === "annotation" && node.properties?.encoding === "application/x-tex") {
    return (node.children ?? []).map((c) => c.value ?? "").join("");
  }
  for (const c of node.children ?? []) {
    const r = latexOf(c);
    if (r !== null) return r;
  }
  return null;
}

/** Byte UTF-8 apa adanya (SVG/teks tanpa menambah baris baru). */
export const textBytesOf = (text: string): Uint8Array => new TextEncoder().encode(text);

const FENCE = "`".repeat(3);
/** Blok kode Markdown berpagar. */
export const fenced = (lang: string, body: string): string => `${FENCE}${lang}
${body}
${FENCE}`;
