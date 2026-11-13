import { describe, expect, test } from "vitest";
import {
  classify,
  decodeText,
  defaultMode,
  hasRawMode,
  kindFromName,
  languageFor,
  locateInRoots,
  mimeFor,
  relativeToRoot,
  sniffKind,
} from "./kinds";
import { type DocPrefs, DocStore, docId, type FileSource } from "./store";

describe("jenis dokumen", () => {
  test.each([
    ["a.md", "markdown"],
    ["README.MD", "markdown"],
    ["g.svg", "svg"],
    ["f.PNG", "image"],
    ["d.pdf", "pdf"],
    ["t.csv", "csv"],
    ["t.tsv", "tsv"],
    ["x.json", "json"],
    ["x.jsonc", "json"],
    ["i.html", "html"],
    ["a.mp3", "audio"],
    ["v.mp4", "video"],
    ["main.rs", "code"],
    ["Dockerfile", "code"],
    ["x.docx", "binary"],
    ["arsip.zip", "binary"],
  ])("%s → %s", (name, kind) => {
    expect(kindFromName(name)).toBe(kind);
  });

  test("nama tak dikenal: null, lalu isi menentukan (NUL → biner, selain itu kode)", () => {
    expect(kindFromName("LICENSE")).toBeNull();
    expect(kindFromName("data.zzz")).toBeNull();
    expect(classify("LICENSE", new TextEncoder().encode("MIT"))).toBe("code");
    expect(classify("data.zzz", new Uint8Array([1, 2, 0, 3]))).toBe("binary");
    expect(sniffKind(new Uint8Array([]))).toBe("code");
    expect(classify("LICENSE", null)).toBe("code");
  });

  test("kunci prototipe tidak dianggap ekstensi", () => {
    expect(kindFromName("a.constructor")).toBeNull();
    expect(languageFor("a.__proto__")).toBe("plaintext");
  });

  test("bahasa editor dan MIME", () => {
    expect(languageFor("src/app.tsx")).toBe("typescript");
    expect(languageFor("Dockerfile")).toBe("dockerfile");
    expect(languageFor("x.unknown")).toBe("plaintext");
    expect(mimeFor("a.png")).toBe("image/png");
    expect(mimeFor("a.svg")).toBe("image/svg+xml");
    expect(mimeFor("a.zzz")).toBe("application/octet-stream");
  });

  test("mode: kode mentah; format kaya render; hanya format dua-tampilan punya mode mentah", () => {
    expect(defaultMode("code")).toBe("raw");
    expect(defaultMode("markdown")).toBe("render");
    expect(
      hasRawMode("markdown") && hasRawMode("json") && hasRawMode("csv") && hasRawMode("html") && hasRawMode("svg"),
    ).toBe(true);
    expect(hasRawMode("image") || hasRawMode("pdf") || hasRawMode("code")).toBe(false);
  });
});

describe("dekode teks", () => {
  test("UTF-8 dengan BOM dibuang tetapi newline akhir dipertahankan", () => {
    const bytes = new Uint8Array([0xef, 0xbb, 0xbf, ...new TextEncoder().encode("halo\r\nbaris\n")]);
    expect(decodeText(bytes)).toMatchObject({
      text: "halo\r\nbaris\n",
      encoding: "utf-8",
      bom: true,
      truncated: false,
    });
  });
  test("UTF-16 LE/BE dikenali dari BOM", () => {
    const le = new Uint8Array([0xff, 0xfe, 0x68, 0x00, 0x69, 0x00]);
    const be = new Uint8Array([0xfe, 0xff, 0x00, 0x68, 0x00, 0x69]);
    expect(decodeText(le)).toMatchObject({ text: "hi", encoding: "utf-16le" });
    expect(decodeText(be)).toMatchObject({ text: "hi", encoding: "utf-16be" });
  });
  test("byte tidak valid diganti U+FFFD (tidak melempar); isi besar dipotong dan ditandai", () => {
    expect(decodeText(new Uint8Array([0x68, 0xff, 0x69])).text).toBe("h\uFFFDi");
    const r = decodeText(new Uint8Array(100).fill(0x61), 10);
    expect(r.truncated).toBe(true);
    expect(r.text).toHaveLength(10);
  });
});

describe("path", () => {
  test("relatif terhadap root; tidak peka huruf di Windows; pemisah dinormalkan; bukan di dalam → null", () => {
    expect(relativeToRoot("C:\\Work\\alpha", "C:\\Work\\alpha\\src\\a.ts")).toBe("src/a.ts");
    expect(relativeToRoot("C:\\Work\\Alpha\\", "c:/work/alpha/README.md")).toBe("README.md");
    expect(relativeToRoot("/home/a/p", "/home/a/p/x/y")).toBe("x/y");
    expect(relativeToRoot("/home/a/p", "/home/a/p")).toBeNull();
    expect(relativeToRoot("/home/a/p", "/home/a/pp/x")).toBeNull();
    expect(relativeToRoot("C:\\Work\\alpha", "C:\\Work\\beta\\a.ts")).toBeNull();
    expect(relativeToRoot("/a/b", "/a/b/../c")).toBeNull();
  });
  test("locateInRoots memilih root paling spesifik", () => {
    const roots = ["C:\\Work", "C:\\Work\\alpha"];
    expect(locateInRoots(roots, "C:\\Work\\alpha\\a.ts")).toEqual({ root: "C:\\Work\\alpha", path: "a.ts" });
    expect(locateInRoots(roots, "C:\\Work\\beta\\b.ts")).toEqual({ root: "C:\\Work", path: "beta/b.ts" });
    expect(locateInRoots(roots, "D:\\x\\y.ts")).toBeNull();
  });
});

describe("DocStore", () => {
  const enc = (s: string) => new TextEncoder().encode(s);
  const flush = () => new Promise((r) => setTimeout(r, 0));
  const memPrefs = (): DocPrefs & { data: Record<string, string> } => {
    const data: Record<string, string> = {};
    return {
      data,
      get: (k) => data[k] ?? null,
      set: (k, v) => {
        data[k] = v;
      },
    };
  };
  function source(files: Record<string, string | Error> = {}) {
    const calls: string[] = [];
    const src: FileSource & { calls: string[]; files: Record<string, string | Error>; gate: Map<string, () => void> } =
      {
        calls,
        files,
        gate: new Map(),
        async read(root, path) {
          calls.push(`${root}|${path}`);
          const f = src.files[path];
          if (f === undefined) throw new Error("berkas tidak ditemukan");
          if (f instanceof Error) throw f;
          return enc(f);
        },
      };
    return src;
  }

  test("buka memuat isi, mengenali jenis, dan tidak menggandakan tab", async () => {
    const src = source({ "a.md": "# Halo", "b.json": "{}" });
    const store = new DocStore({ source: src });
    const id = store.open("R", "a.md");
    expect(store.state().tabs[0]).toMatchObject({ kind: "markdown", mode: "render", status: "loading" });
    await flush();
    expect(store.state().tabs[0]).toMatchObject({ status: "ready", size: 6, version: 1 });
    expect(new TextDecoder().decode(store.content(id) as Uint8Array)).toBe("# Halo");
    expect(store.open("R", "./a.md")).toBe(id);
    expect(store.open("R", "a\\b.json".replace("\\", "/"))).toBeDefined();
    expect(store.state().tabs).toHaveLength(2);
    expect(src.calls.filter((c) => c.endsWith("a.md"))).toHaveLength(1);
  });

  test("galat baca ditampilkan di tab (bukan spinner abadi) dan reload memulihkan", async () => {
    const src = source({});
    const store = new DocStore({ source: src });
    const id = store.open("R", "hilang.txt");
    await flush();
    expect(store.state().tabs[0]).toMatchObject({ status: "error", error: "berkas tidak ditemukan" });
    src.files["hilang.txt"] = "ada";
    store.reload(id);
    await flush();
    expect(store.state().tabs[0]).toMatchObject({ status: "ready", error: null, version: 1 });
  });

  test("respons basi dibuang bila tab ditutup atau dimuat ulang lebih baru", async () => {
    let release: (b: Uint8Array) => void = () => {};
    let n = 0;
    const src: FileSource = {
      read: () => {
        n++;
        if (n === 1) return new Promise<Uint8Array>((r) => (release = r));
        return Promise.resolve(enc("baru"));
      },
    };
    const store = new DocStore({ source: src });
    const id = store.open("R", "a.txt");
    store.reload(id);
    await flush();
    release(enc("lama"));
    await flush();
    expect(new TextDecoder().decode(store.content(id) as Uint8Array)).toBe("baru");
    store.close(id);
    expect(store.state().tabs).toEqual([]);
  });

  test("menutup tab aktif memilih tetangga; tab terakhir → tanpa aktif", async () => {
    const src = source({ "a.txt": "a", "b.txt": "b", "c.txt": "c" });
    const store = new DocStore({ source: src });
    const a = store.open("R", "a.txt");
    const b = store.open("R", "b.txt");
    const c = store.open("R", "c.txt");
    await flush();
    store.close(c);
    expect(store.state().activeId).toBe(b);
    store.close(a);
    expect(store.state().activeId).toBe(b);
    store.close(b);
    expect(store.state()).toEqual({ tabs: [], activeId: null });
  });

  test("cache dibatasi: tab lama dibuang isinya dan dimuat ulang saat diaktifkan", async () => {
    const src = source({ "1.txt": "1", "2.txt": "2", "3.txt": "3" });
    const store = new DocStore({ source: src, maxCached: 2 });
    const t1 = store.open("R", "1.txt");
    await flush();
    store.open("R", "2.txt");
    await flush();
    store.open("R", "3.txt");
    await flush();
    expect(store.content(t1)).toBeNull();
    expect(store.state().tabs[0]?.status).toBe("idle");
    store.activate(t1);
    await flush();
    expect(new TextDecoder().decode(store.content(t1) as Uint8Array)).toBe("1");
    expect(src.calls.filter((c) => c.endsWith("1.txt"))).toHaveLength(2);
  });

  test("mode dan wrap: hanya format dua-tampilan yang bisa berganti mode; tersimpan dan pulih", async () => {
    const src = source({ "a.md": "x", "b.ts": "y" });
    const prefs = memPrefs();
    const store = new DocStore({ source: src, prefs });
    const md = store.open("R", "a.md");
    const ts = store.open("R", "b.ts");
    await flush();
    store.setMode(md, "raw");
    store.setMode(ts, "render");
    store.toggleWrap(ts);
    expect(store.state().tabs.find((t) => t.id === md)?.mode).toBe("raw");
    expect(store.state().tabs.find((t) => t.id === ts)).toMatchObject({ mode: "raw", wrap: true });

    const again = new DocStore({ source: src, prefs });
    again.restore();
    expect(again.state().tabs.map((t) => [t.path, t.mode, t.wrap])).toEqual([
      ["a.md", "raw", false],
      ["b.ts", "raw", true],
    ]);
    expect(again.state().activeId).toBe(docId("R", "b.ts"));
    await flush();
    expect(again.state().tabs[1]?.status).toBe("ready"); // tab aktif dimuat, yang lain idle
    expect(again.state().tabs[0]?.status).toBe("idle");
  });

  test("restore mengabaikan data rusak", () => {
    const prefs = memPrefs();
    prefs.set("zandor.docTabs", "{rusak");
    const s = new DocStore({ source: source(), prefs });
    s.restore();
    expect(s.state().tabs).toEqual([]);
    prefs.set(
      "zandor.docTabs",
      JSON.stringify({ active: "x", tabs: [{ root: 1 }, null, { root: "R", path: "a.md" }] }),
    );
    const t = new DocStore({ source: source({ "a.md": "x" }), prefs });
    t.restore();
    expect(t.state().tabs).toHaveLength(1);
  });

  test("jumlah tab dibatasi", async () => {
    const files: Record<string, string> = {};
    for (let i = 0; i < 10; i++) files[`f${i}.txt`] = "x";
    const store = new DocStore({ source: source(files), maxTabs: 4 });
    for (let i = 0; i < 10; i++) store.open("R", `f${i}.txt`);
    await flush();
    expect(store.state().tabs).toHaveLength(4);
    expect(store.state().activeId).toBe(docId("R", "f9.txt"));
  });
});
