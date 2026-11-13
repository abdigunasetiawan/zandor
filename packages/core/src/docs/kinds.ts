// Jenis dokumen Panel 3 (Fase 7): deteksi dari nama berkas (dan isi bila nama tak dikenal), bahasa editor, MIME, dan
// utilitas path. Murni dan terdokumentasi lewat test; renderer sebenarnya ada di apps/desktop.

export type DocKind =
  | "code"
  | "markdown"
  | "svg"
  | "image"
  | "pdf"
  | "csv"
  | "tsv"
  | "json"
  | "html"
  | "audio"
  | "video"
  | "docx"
  | "xlsx"
  | "pptx"
  | "notebook"
  | "binary";

export type DocMode = "render" | "raw";

const EXT_KIND: Record<string, DocKind> = {
  md: "markdown",
  markdown: "markdown",
  mdx: "markdown",
  svg: "svg",
  png: "image",
  jpg: "image",
  jpeg: "image",
  gif: "image",
  webp: "image",
  bmp: "image",
  ico: "image",
  avif: "image",
  pdf: "pdf",
  csv: "csv",
  tsv: "tsv",
  json: "json",
  jsonc: "json",
  geojson: "json",
  html: "html",
  htm: "html",
  mp3: "audio",
  wav: "audio",
  ogg: "audio",
  oga: "audio",
  m4a: "audio",
  flac: "audio",
  mp4: "video",
  webm: "video",
  mov: "video",
  m4v: "video",
  ogv: "video",
  // Office/Notebook (Fase 11): penampil best-effort; format lama (doc/xls/ppt) tetap biner → "buka di aplikasi lain"
  docx: "docx",
  xlsx: "xlsx",
  pptx: "pptx",
  ipynb: "notebook",
  doc: "binary",
  xls: "binary",
  ppt: "binary",
  zip: "binary",
  gz: "binary",
  tar: "binary",
  "7z": "binary",
  exe: "binary",
  dll: "binary",
  wasm: "binary",
  bin: "binary",
  sqlite: "binary",
  db: "binary",
};

/** Bahasa Monaco menurut ekstensi. Tidak ada → "plaintext". */
const EXT_LANG: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  mts: "typescript",
  cts: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  jsonc: "json",
  md: "markdown",
  markdown: "markdown",
  html: "html",
  htm: "html",
  css: "css",
  scss: "scss",
  less: "less",
  xml: "xml",
  svg: "xml",
  yaml: "yaml",
  yml: "yaml",
  toml: "ini",
  ini: "ini",
  py: "python",
  rs: "rust",
  go: "go",
  java: "java",
  kt: "kotlin",
  c: "c",
  h: "c",
  cpp: "cpp",
  cc: "cpp",
  hpp: "cpp",
  cs: "csharp",
  rb: "ruby",
  php: "php",
  sh: "shell",
  bash: "shell",
  zsh: "shell",
  ps1: "powershell",
  bat: "bat",
  cmd: "bat",
  sql: "sql",
  swift: "swift",
  lua: "lua",
  r: "r",
  dockerfile: "dockerfile",
  graphql: "graphql",
  gql: "graphql",
  csv: "plaintext",
  tsv: "plaintext",
  txt: "plaintext",
  log: "plaintext",
};

const NAME_LANG: Record<string, string> = {
  dockerfile: "dockerfile",
  makefile: "plaintext",
  ".gitignore": "plaintext",
  ".env": "ini",
};

export const fileName = (path: string): string => path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;

export function extOf(path: string): string {
  const n = fileName(path);
  const dot = n.lastIndexOf(".");
  return dot <= 0 ? "" : n.slice(dot + 1).toLowerCase();
}

/** Jenis dari nama saja; `null` bila tidak dikenal (pemanggil bisa mengintip isi lewat `sniffKind`). */
export function kindFromName(path: string): DocKind | null {
  const ext = extOf(path);
  const k = Object.hasOwn(EXT_KIND, ext) ? EXT_KIND[ext] : undefined;
  if (k) return k;
  if (Object.hasOwn(EXT_LANG, ext) || Object.hasOwn(NAME_LANG, fileName(path).toLowerCase())) return "code";
  return null;
}

/** Berkas tak dikenal: ada byte NUL di 8 KB pertama → biner; selain itu teks/kode. */
export function sniffKind(bytes: Uint8Array): DocKind {
  const n = Math.min(bytes.length, 8192);
  for (let i = 0; i < n; i++) if (bytes[i] === 0) return "binary";
  return "code";
}

export function classify(path: string, bytes: Uint8Array | null): DocKind {
  return kindFromName(path) ?? (bytes ? sniffKind(bytes) : "code");
}

export function languageFor(path: string): string {
  const name = fileName(path).toLowerCase();
  if (Object.hasOwn(NAME_LANG, name)) return NAME_LANG[name] as string;
  const ext = extOf(path);
  return Object.hasOwn(EXT_LANG, ext) ? (EXT_LANG[ext] as string) : "plaintext";
}

const MIME: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  bmp: "image/bmp",
  ico: "image/x-icon",
  avif: "image/avif",
  svg: "image/svg+xml",
  pdf: "application/pdf",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  ogg: "audio/ogg",
  oga: "audio/ogg",
  m4a: "audio/mp4",
  flac: "audio/flac",
  mp4: "video/mp4",
  m4v: "video/mp4",
  webm: "video/webm",
  ogv: "video/ogg",
  mov: "video/quicktime",
  html: "text/html",
  htm: "text/html",
};

export const mimeFor = (path: string): string => MIME[extOf(path)] ?? "application/octet-stream";

/** Kind yang punya dua tampilan (render + mentah). */
export function hasRawMode(kind: DocKind): boolean {
  return (
    kind === "markdown" ||
    kind === "svg" ||
    kind === "csv" ||
    kind === "tsv" ||
    kind === "json" ||
    kind === "html" ||
    kind === "notebook"
  );
}

/** Kind yang hanya bisa mentah (editor teks). */
export const isTextKind = (kind: DocKind): boolean => kind === "code" || hasRawMode(kind);

export function defaultMode(kind: DocKind): DocMode {
  return kind === "code" ? "raw" : "render";
}

// ---------- teks ----------

export type DecodedText = {
  text: string;
  encoding: "utf-8" | "utf-16le" | "utf-16be";
  bom: boolean;
  truncated: boolean;
};

export const MAX_TEXT_BYTES = 5 * 1024 * 1024;

/** Dekode teks dengan mengenali BOM UTF-8/16; memotong di `MAX_TEXT_BYTES` (ditandai `truncated`). */
export function decodeText(bytes: Uint8Array, maxBytes = MAX_TEXT_BYTES): DecodedText {
  const truncated = bytes.length > maxBytes;
  let slice = truncated ? bytes.subarray(0, maxBytes) : bytes;
  let encoding: DecodedText["encoding"] = "utf-8";
  let bom = false;
  if (slice[0] === 0xef && slice[1] === 0xbb && slice[2] === 0xbf) {
    slice = slice.subarray(3);
    bom = true;
  } else if (slice[0] === 0xff && slice[1] === 0xfe) {
    slice = slice.subarray(2);
    encoding = "utf-16le";
    bom = true;
  } else if (slice[0] === 0xfe && slice[1] === 0xff) {
    slice = slice.subarray(2);
    encoding = "utf-16be";
    bom = true;
  }
  // `fatal: false` mengganti byte tak valid dengan U+FFFD, bukan melempar.
  const text = new TextDecoder(encoding, { fatal: false }).decode(slice);
  return { text, encoding, bom, truncated };
}

// ---------- path ----------

const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");

/** `absolute` di dalam `root` → path relatif (pemisah `/`); selain itu null. Windows tidak peka huruf besar/kecil. */
export function relativeToRoot(root: string, absolute: string): string | null {
  const r = norm(root);
  const a = norm(absolute);
  const win = /^[A-Za-z]:/.test(r);
  const rc = win ? r.toLowerCase() : r;
  const ac = win ? a.toLowerCase() : a;
  if (ac === rc || !ac.startsWith(`${rc}/`)) return null;
  const rel = a.slice(r.length + 1);
  return rel.split("/").some((s) => s === "..") ? null : rel;
}

/** Root terpanjang (paling spesifik) yang memuat `absolute`, beserta path relatifnya. */
export function locateInRoots(roots: string[], absolute: string): { root: string; path: string } | null {
  let best: { root: string; path: string } | null = null;
  for (const root of roots) {
    const rel = relativeToRoot(root, absolute);
    if (rel !== null && (!best || root.length > best.root.length)) best = { root, path: rel };
  }
  return best;
}

export const isAbsolutePath = (p: string): boolean =>
  /^[A-Za-z]:[\\/]/.test(p) || p.startsWith("/") || p.startsWith("\\\\");
