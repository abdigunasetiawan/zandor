// Tiga sesi DEMO bawaan (disematkan, permanen, hanya-baca) untuk melihat apa yang bisa ditampilkan aplikasi: trajectory agent
// (berpikir, tool call, pencarian web, memori, skill), render kaya (teks, tabel, kode, rumus, diagram, chart, gambar, kartu
// berkas), serta kartu izin dan formulir pertanyaan. Isinya STATIS dan diberi label "Demo" di antarmuka — bukan percakapan
// nyata, tidak dikirim ke engine, dan tidak disimpan di riwayat sesi Anda (ADR-043).
import {
  type ChatMessage,
  initialChatState,
  type MessagePart,
  type PermissionRequest,
  type QuestionRequest,
  type SessionChat,
  type Todo,
} from "@zandor/core";
import { DEMO_CHART_IMAGE, DEMO_DIAGRAM_IMAGE } from "./demo-assets";

/** Kunci "project" untuk sesi demo (bukan id project nyata maupun CHAT_KEY). */
export const DEMO_KEY = "demo";

export type DemoSessionDef = {
  id: string;
  /** Kunci i18n judul dan keterangan. */
  titleKey: "demo.trajectory.title" | "demo.render.title" | "demo.ask.title";
  subtitleKey: "demo.trajectory.sub" | "demo.render.sub" | "demo.ask.sub";
  build: () => SessionChat;
  /** Izin dan pertanyaan awal (bisa dijawab di UI demo, lalu di-reset). */
  initialPermissions: PermissionRequest[];
  initialQuestions: QuestionRequest[];
};

const F = "```";
const T0 = Date.UTC(2026, 10, 14, 9, 0, 0);
const pad = (n: number) => String(n).padStart(4, "0");

type Ctx = { sid: string; n: number };
const next = (c: Ctx) => pad(++c.n);

function user(c: Ctx, text: string, t: number): ChatMessage {
  const id = `msg_${next(c)}`;
  return {
    info: { id, sessionID: c.sid, role: "user", time: { created: T0 + t, completed: T0 + t } },
    parts: [{ id: `prt_${next(c)}`, messageID: id, sessionID: c.sid, type: "text", text, synthetic: false }],
  };
}

type PartSpec =
  | { k: "text"; text: string }
  | { k: "reasoning"; text: string }
  | {
      k: "tool";
      tool: string;
      status?: "completed" | "error" | "pending" | "running";
      input: Record<string, unknown>;
      output?: string;
      error?: string;
      title?: string;
      diff?: string;
      ms?: number;
    }
  | { k: "file"; filename: string; mime: string; url: string };

function assistant(c: Ctx, parts: PartSpec[], t: number, durationMs: number): ChatMessage {
  const id = `msg_${next(c)}`;
  const out: MessagePart[] = parts.map((p) => {
    const base = { id: `prt_${next(c)}`, messageID: id, sessionID: c.sid };
    if (p.k === "text") return { ...base, type: "text", text: p.text, synthetic: false };
    if (p.k === "reasoning") return { ...base, type: "reasoning", text: p.text };
    if (p.k === "file") return { ...base, type: "file", mime: p.mime, filename: p.filename, url: p.url };
    return {
      ...base,
      type: "tool",
      tool: p.tool,
      callID: `call_${base.id}`,
      state: {
        status: p.status ?? "completed",
        input: p.input,
        ...(p.output !== undefined ? { output: p.output } : {}),
        ...(p.error !== undefined ? { error: p.error } : {}),
        ...(p.title ? { title: p.title } : {}),
        ...(p.diff ? { metadata: { diff: p.diff } } : {}),
        time: { start: T0 + t, end: T0 + t + (p.ms ?? 400) },
      },
    };
  });
  return {
    info: {
      id,
      sessionID: c.sid,
      role: "assistant",
      time: { created: T0 + t, completed: T0 + t + durationMs },
      providerID: "demo",
      modelID: "demo-model",
      finish: "stop",
      tokens: { input: 18240, output: 1320, reasoning: 640, total: 20200 },
      cost: 0,
    },
    parts: out,
  };
}

function session(sid: string, messages: ChatMessage[], todos: Todo[] = []): SessionChat {
  return {
    ...initialChatState(sid),
    loaded: true,
    messages,
    todos,
    directory: "",
    sending: false,
    loading: false,
    reconnecting: false,
  };
}

// ---------------------------------------------------------------------------------------------------------------
// 1) Trajectory & tools
// ---------------------------------------------------------------------------------------------------------------

const DIFF_SIDEBAR = `--- a/apps/desktop/src/shell/Sidebar.tsx
+++ b/apps/desktop/src/shell/Sidebar.tsx
@@ -41,7 +41,7 @@ export function Sidebar() {
   const state = useProjects();
-  const selectedProject = state.projects.find((p) => p.id === state.selection.key);
+  const selectedProject = state.projects.find((p) => p.id === state.selection.key) ?? null;
   return (
     <div className="sidebar">`;

function trajectory(): SessionChat {
  const sid = "demo-trajectory";
  const c: Ctx = { sid, n: 0 };
  const todos: Todo[] = [
    { content: "Reproduksi kegagalan build di CI", status: "completed", priority: "high" },
    { content: "Cari penyebab di kode dan catatan sebelumnya", status: "completed", priority: "high" },
    { content: "Perbaiki tipe yang tidak aman", status: "completed", priority: "medium" },
    { content: "Jalankan tes dan build ulang", status: "completed", priority: "medium" },
    { content: "Catat pelajaran ke memori", status: "in_progress", priority: "low" },
  ];
  return session(
    sid,
    [
      user(c, "Build di CI gagal sejak kemarin. Cari penyebabnya, perbaiki, lalu catat pelajarannya.", 0),
      assistant(
        c,
        [
          {
            k: "reasoning",
            text: "Pengguna ingin penyebab kegagalan build CI. Langkah yang masuk akal: (1) muat skill debugging agar ada urutan kerja baku, (2) cek memori apakah masalah serupa pernah terjadi, (3) reproduksi lokal dengan menjalankan build, (4) telusuri pesan galat ke kode, (5) perbaiki seminimal mungkin, (6) verifikasi dengan tes, lalu (7) simpan pelajaran. Saya hindari mengubah konfigurasi CI sebelum memastikan galat bukan dari kode.",
          },
          {
            k: "tool",
            tool: "skill",
            input: { name: "debugging-ci" },
            output:
              "Skill dimuat: debugging-ci\nUrutan: reproduksi → isolasi → perbaiki → verifikasi → catat.\nCatatan: jangan menebak; baca pesan galat pertama, bukan yang terakhir.",
            title: "debugging-ci",
          },
          {
            k: "tool",
            tool: "memory_search",
            input: { query: "build gagal CI tipe undefined" },
            output:
              "2 memori cocok:\n• [project] 'noUncheckedIndexedAccess' membuat akses array bertipe T | undefined (dicatat 3 hari lalu)\n• [global] 'Periksa galat TS pertama dulu; sisanya sering turunan' ",
            title: "2 memori ditemukan",
          },
          {
            k: "tool",
            tool: "bash",
            input: { command: "bun run build", description: "Reproduksi build" },
            output:
              "$ tsc -b && vite build\napps/desktop/src/shell/Sidebar.tsx(44,9): error TS2322: Type 'Project | undefined' is not assignable to type 'Project | null'.\nFound 1 error in apps/desktop/src/shell/Sidebar.tsx:44",
            ms: 5200,
          },
          {
            k: "tool",
            tool: "grep",
            input: { pattern: "selectedProject", path: "apps/desktop/src" },
            output:
              "apps/desktop/src/shell/Sidebar.tsx:44:  const selectedProject = state.projects.find(...)\napps/desktop/src/shell/Sidebar.tsx:118: content: <FilesTree project={selectedProject} />,",
          },
          {
            k: "tool",
            tool: "read",
            input: { filePath: "apps/desktop/src/shell/Sidebar.tsx", offset: 38, limit: 12 },
            output: `${F}tsx\nexport function Sidebar() {\n  const { t } = useI18n();\n  const state = useProjects();\n  const [query, setQuery] = useState("");\n  const selectedProject = state.projects.find((p) => p.id === state.selection.key);\n${F}`,
          },
          {
            k: "tool",
            tool: "edit",
            input: {
              filePath: "apps/desktop/src/shell/Sidebar.tsx",
              oldString: "state.selection.key);",
              newString: "state.selection.key) ?? null;",
            },
            output: "Edit diterapkan.",
            diff: DIFF_SIDEBAR,
          },
          {
            k: "tool",
            tool: "websearch",
            input: { query: "TS2322 Array.find returns undefined not assignable null" },
            output:
              "1. Array.prototype.find — MDN: mengembalikan undefined bila tidak ada elemen yang cocok.\n2. TypeScript Handbook — Narrowing: gunakan ?? untuk mengganti undefined dengan nilai bawaan.\n3. Stack Overflow — 'Type undefined is not assignable to null': samakan union atau normalisasi dengan ??.",
            title: "3 hasil",
            ms: 1800,
          },
          {
            k: "tool",
            tool: "webfetch",
            input: {
              url: "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/find",
            },
            output:
              "# Array.prototype.find()\nMetode find() mengembalikan elemen pertama yang memenuhi fungsi pengujian. Bila tidak ada, mengembalikan undefined.",
            ms: 2300,
          },
          {
            k: "tool",
            tool: "bash",
            input: {
              command: "curl -s https://api.ci-status.invalid/v1/last",
              description: "Cek status CI (contoh galat)",
            },
            status: "error",
            error: "curl: (6) Could not resolve host: api.ci-status.invalid",
            ms: 120,
          },
          {
            k: "tool",
            tool: "bash",
            input: { command: "bun x vitest run && bun run build", description: "Verifikasi" },
            output:
              " Test Files  41 passed (41)\n      Tests  650 passed (650)\n$ tsc -b && vite build\n✓ built in 3.5s",
            ms: 61000,
          },
          {
            k: "tool",
            tool: "todowrite",
            input: { todos: todos.map((t) => ({ content: t.content, status: t.status })) },
            output: "5 tugas diperbarui",
          },
          {
            k: "tool",
            tool: "memory_write",
            input: {
              scope: "project",
              text: "Hasil Array.find bertipe T | undefined; normalisasi dengan ?? null saat state bertipe null.",
            },
            output: "Memori disimpan (project).",
            title: "Memori disimpan",
          },
          {
            k: "text",
            text: `## Selesai\n\nBuild gagal karena **\`Array.find\` mengembalikan \`undefined\`**, sedangkan prop \`FilesTree\` bertipe \`Project | null\`.\n\n- [x] Direproduksi lokal (\`bun run build\`)\n- [x] Penyebab: \`Sidebar.tsx:44\`\n- [x] Perbaikan: \`?? null\`\n- [x] 650 tes lulus, build hijau\n- [x] Pelajaran dicatat ke memori project\n\n> Tip: buka kartu tool di atas untuk melihat input, output, dan diff-nya.`,
          },
        ],
        3_000,
        96_000,
      ),
    ],
    todos,
  );
}

// ---------------------------------------------------------------------------------------------------------------
// 2) Render kaya
// ---------------------------------------------------------------------------------------------------------------

function render(): SessionChat {
  const sid = "demo-render";
  const c: Ctx = { sid, n: 0 };
  return session(sid, [
    user(
      c,
      "Tampilkan semua jenis konten yang bisa kamu render: teks, tabel, kode, rumus, diagram, chart, gambar, kartu berkas.",
      0,
    ),
    assistant(
      c,
      [
        {
          k: "reasoning",
          text: "Saya akan menyusun contoh per jenis konten supaya mudah dibandingkan: teks beserta format inline, tabel, blok kode berwarna dengan tombol salin, rumus KaTeX, diagram Mermaid, chart (pie dan XY), gambar, dan kartu berkas.",
        },
        {
          k: "text",
          text: `## 1. Teks dan format\n\nIni paragraf dengan **tebal**, *miring*, ~~coret~~, \`kode inline\`, dan [tautan aman](https://example.com).\n\n1. Daftar bernomor\n2. Dengan butir kedua\n   - sub-butir bertingkat\n   - dan satu lagi\n\n- [x] Daftar tugas selesai\n- [ ] Daftar tugas belum\n\n> Kutipan: tampilan ini dirender dari Markdown, bukan HTML mentah.\n\n---`,
        },
        {
          k: "text",
          text: `## 2. Tabel\n\n| Provider | Model | Konteks | Harga / 1M token | Status |\n| --- | --- | ---: | ---: | :---: |\n| Anthropic | claude-sonnet | 200k | $3 / $15 | ✅ |\n| OpenAI | gpt-5 | 400k | $1,25 / $10 | ✅ |\n| Google | gemini-flash | 1M | $0,30 / $2,50 | ✅ |\n| Lokal | llama-3 8B | 8k | gratis | ⚠️ lambat |`,
        },
        {
          k: "text",
          text: `## 3. Kode (berwarna, ada tombol salin)\n\n${F}ts\ninterface Pesan {\n  id: string;\n  peran: "user" | "assistant";\n  isi: string;\n}\n\nexport const ringkas = (p: Pesan[]): string =>\n  p.map((x) => \`\${x.peran}: \${x.isi.slice(0, 40)}\`).join("\\n");\n${F}\n\n${F}python\ndef fib(n: int) -> list[int]:\n    a, b = 0, 1\n    out = []\n    for _ in range(n):\n        out.append(a)\n        a, b = b, a + b\n    return out\n${F}\n\n${F}bash\nbun install && bun run dev\n${F}\n\n${F}json\n{ "nama": "zandor", "versi": "0.0.36", "aktif": true }\n${F}\n\n${F}diff\n- const x = lama();\n+ const x = baru();\n${F}`,
        },
        {
          k: "text",
          text: `## 4. Rumus (KaTeX)\n\nInline: $E = mc^2$. Blok:\n\n$$\n\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}\n$$\n\n$$\n\\bar{x} = \\frac{1}{n}\\sum_{i=1}^{n} x_i,\\qquad \\sigma^2 = \\frac{1}{n}\\sum_{i=1}^{n}(x_i-\\bar{x})^2\n$$`,
        },
        {
          k: "text",
          text: `## 5. Diagram (Mermaid)\n\n${F}mermaid\nflowchart LR\n  U[Pengguna] --> A[Aplikasi]\n  A --> E[Engine opencode]\n  E --> P{Provider}\n  P -->|HTTPS| M1[Model A]\n  P -->|lokal| M2[Model B]\n${F}\n\n${F}mermaid\nsequenceDiagram\n  participant U as Pengguna\n  participant A as Aplikasi\n  participant E as Engine\n  U->>A: Kirim pesan\n  A->>E: prompt\n  E-->>A: stream respons\n  A-->>U: tampil bertahap\n${F}`,
        },
        {
          k: "text",
          text: `## 6. Chart\n\n${F}mermaid\npie showData title Distribusi token sesi\n  "Input" : 62\n  "Output" : 28\n  "Reasoning" : 10\n${F}\n\n${F}mermaid\nxychart-beta\n  title "Waktu build per percobaan (detik)"\n  x-axis [p1, p2, p3, p4, p5]\n  y-axis "detik" 0 --> 80\n  bar [68, 55, 43, 33, 25]\n  line [68, 55, 43, 33, 25]\n${F}`,
        },
        {
          k: "text",
          text: `## 7. Gambar\n\nGambar disisipkan lewat Markdown (hanya data URI yang dimuat; gambar dari internet tidak diambil demi keamanan):\n\n![Grafik batang waktu build](${DEMO_CHART_IMAGE})`,
        },
        { k: "file", filename: "arsitektur.svg", mime: "image/svg+xml", url: DEMO_DIAGRAM_IMAGE },
        {
          k: "text",
          text: "## 8. Kartu berkas\n\nBerkas yang dilampirkan/dihasilkan tampil sebagai kartu (jenis ikon mengikuti ekstensi):",
        },
        { k: "file", filename: "laporan-q3.pdf", mime: "application/pdf", url: "demo://laporan-q3.pdf" },
        {
          k: "file",
          filename: "anggaran.xlsx",
          mime: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
          url: "demo://anggaran.xlsx",
        },
        {
          k: "file",
          filename: "proposal.docx",
          mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
          url: "demo://proposal.docx",
        },
        { k: "file", filename: "sidebar.tsx", mime: "text/typescript", url: "demo://sidebar.tsx" },
        { k: "file", filename: "data.csv", mime: "text/csv", url: "demo://data.csv" },
      ],
      2_000,
      14_000,
    ),
  ]);
}

// ---------------------------------------------------------------------------------------------------------------
// 3) Izin & pertanyaan
// ---------------------------------------------------------------------------------------------------------------

const DIFF_AUTH = `--- a/src/auth/session.ts
+++ b/src/auth/session.ts
@@ -12,6 +12,10 @@
 export function createSession(user: User) {
-  return { id: newId(), user };
+  return {
+    id: newId(),
+    user,
+    expiresAt: Date.now() + SESSION_TTL_MS,
+  };
 }`;

const PERMISSIONS: PermissionRequest[] = [
  {
    id: "demo-perm-edit",
    sessionID: "demo-ask",
    permission: "edit",
    patterns: ["src/auth/session.ts"],
    metadata: { diff: DIFF_AUTH },
    always: ["src/auth/*"],
  },
  {
    id: "demo-perm-bash",
    sessionID: "demo-ask",
    permission: "bash",
    patterns: ["bun run migrate --dry-run"],
    metadata: {},
    always: ["bun run migrate *"],
  },
  {
    id: "demo-perm-external",
    sessionID: "demo-ask",
    permission: "external_directory",
    patterns: ["C:/data/backup/*"],
    metadata: {},
    always: [],
  },
];

const QUESTIONS: QuestionRequest[] = [
  {
    id: "demo-q-1",
    sessionID: "demo-ask",
    questions: [
      {
        header: "Strategi migrasi",
        question: "Bagaimana migrasi database sebaiknya dijalankan?",
        multiple: false,
        custom: true,
        options: [
          { label: "Bertahap (disarankan)", description: "Migrasi per tabel dengan titik pulih di tiap langkah." },
          { label: "Sekali jalan", description: "Lebih cepat tetapi lebih berisiko; perlu jendela pemeliharaan." },
          { label: "Hanya simulasi", description: "Jalankan --dry-run dan laporkan perubahan tanpa menerapkan." },
        ],
      },
      {
        header: "Modul yang disertakan",
        question: "Modul mana saja yang ikut di-refactor? (boleh lebih dari satu)",
        multiple: true,
        custom: false,
        options: [
          { label: "auth", description: "Sesi dan token." },
          { label: "billing", description: "Faktur dan langganan." },
          { label: "notifikasi", description: "Email dan push." },
        ],
      },
      {
        header: "Nama cabang",
        question: "Beri nama cabang git untuk perubahan ini.",
        multiple: false,
        custom: true,
        options: [],
      },
    ],
  },
];

function ask(): SessionChat {
  const sid = "demo-ask";
  const c: Ctx = { sid, n: 0 };
  const todos: Todo[] = [
    { content: "Baca modul auth dan pahami alurnya", status: "completed", priority: "high" },
    { content: "Ubah createSession agar punya masa berlaku", status: "in_progress", priority: "high" },
    { content: "Jalankan migrasi (simulasi dulu)", status: "pending", priority: "medium" },
    { content: "Perbarui tes sesi", status: "pending", priority: "medium" },
    { content: "Buat cabang dan ringkasan perubahan", status: "pending", priority: "low" },
  ];
  return session(
    sid,
    [
      user(c, "Refactor modul auth supaya sesi punya masa berlaku, lalu jalankan migrasi database.", 0),
      assistant(
        c,
        [
          {
            k: "reasoning",
            text: "Perubahan ini menyentuh berkas sumber dan menjalankan perintah shell, jadi agent harus meminta izin. Ada juga keputusan yang bergantung pada preferensi pengguna (strategi migrasi, modul, nama cabang) sehingga agent bertanya lewat formulir, bukan menebak.",
          },
          {
            k: "tool",
            tool: "read",
            input: { filePath: "src/auth/session.ts" },
            output: `${F}ts\nexport function createSession(user: User) {\n  return { id: newId(), user };\n}\n${F}`,
          },
          {
            k: "tool",
            tool: "edit",
            status: "pending",
            input: { filePath: "src/auth/session.ts", newString: "expiresAt: Date.now() + SESSION_TTL_MS" },
            title: "Menunggu izin edit",
          },
          {
            k: "tool",
            tool: "bash",
            status: "pending",
            input: { command: "bun run migrate --dry-run" },
            title: "Menunggu izin bash",
          },
          {
            k: "text",
            text: "Saya menunggu **tiga izin** dan **satu formulir pertanyaan** di bawah ini. Coba jawab: tombolnya bekerja, tetapi karena ini sesi demo jawabannya hanya ditampilkan (tidak dikirim ke engine).",
          },
        ],
        2_000,
        8_000,
      ),
    ],
    todos,
  );
}

export const DEMO_SESSIONS: DemoSessionDef[] = [
  {
    id: "demo-trajectory",
    titleKey: "demo.trajectory.title",
    subtitleKey: "demo.trajectory.sub",
    build: trajectory,
    initialPermissions: [],
    initialQuestions: [],
  },
  {
    id: "demo-render",
    titleKey: "demo.render.title",
    subtitleKey: "demo.render.sub",
    build: render,
    initialPermissions: [],
    initialQuestions: [],
  },
  {
    id: "demo-ask",
    titleKey: "demo.ask.title",
    subtitleKey: "demo.ask.sub",
    build: ask,
    initialPermissions: PERMISSIONS,
    initialQuestions: QUESTIONS,
  },
];

export const demoById = (id: string | null): DemoSessionDef | undefined => DEMO_SESSIONS.find((d) => d.id === id);
