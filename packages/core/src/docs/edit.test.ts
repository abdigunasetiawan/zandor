import { describe, expect, test } from "vitest";
import { DocStore, type FileSource } from "./store";

const enc = (s: string) => new TextEncoder().encode(s);
const tick = () => new Promise((r) => setTimeout(r, 0));

function make(files: Record<string, Uint8Array>, withWrite = true) {
  const disk = new Map(Object.entries(files));
  const mtime = new Map(Object.keys(files).map((k) => [k, 1000]));
  const writes: Array<{ path: string; text: string; expected: number | null }> = [];
  const source: FileSource = {
    async read(_r, p) {
      const v = disk.get(p);
      if (!v) throw new Error("tidak ada");
      return v;
    },
    async stat(_r, p) {
      return { size: disk.get(p)?.length ?? 0, modified: mtime.get(p) ?? 0 };
    },
    ...(withWrite && {
      async write(_r: string, p: string, text: string, expected: number | null) {
        writes.push({ path: p, text, expected });
        if (expected !== null && expected !== mtime.get(p))
          throw new Error("KONFLIK: berkas berubah di disk sejak dibuka");
        disk.set(p, enc(text));
        const m = (mtime.get(p) ?? 0) + 1;
        mtime.set(p, m);
        return m;
      },
    }),
  };
  return { store: new DocStore({ source }), disk, mtime, writes };
}

async function open(store: DocStore, path = "a.ts") {
  const id = store.open("C:/p", path);
  await tick();
  await tick();
  return id;
}

describe("edit berkas (DocStore)", () => {
  test("bisa diedit: berkas kode UTF-8; modified dibaca saat load", async () => {
    const { store } = make({ "a.ts": enc("const a = 1;\n") });
    const id = await open(store);
    expect(store.state().tabs[0]?.modified).toBe(1000);
    expect(store.editBlock(id)).toBeNull();
    expect(store.beginEdit(id)).toBeNull();
    expect(store.state().tabs[0]?.editing).toBe(true);
  });

  test("alasan penolakan: tanpa write, bukan teks, UTF-16, biner, bahkan U+FFFD, terpotong, belum dimuat", async () => {
    const noWrite = make({ "a.ts": enc("x") }, false);
    expect(noWrite.store.beginEdit(await open(noWrite.store))).toBe("noWrite");

    const img = make({ "p.png": Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0, 1, 2]) });
    expect(img.store.beginEdit(await open(img.store, "p.png"))).toBe("kind");

    const u16 = make({ "a.ts": Uint8Array.from([0xff, 0xfe, 0x61, 0x00]) });
    expect(u16.store.beginEdit(await open(u16.store))).toBe("encoding");

    const bad = make({ "a.ts": Uint8Array.from([0x61, 0xff, 0xfe, 0x62]) });
    expect(bad.store.beginEdit(await open(bad.store))).toBe("encoding");

    const nul = make({ "a.ts": Uint8Array.from([0x61, 0x00, 0x62]) });
    expect(nul.store.beginEdit(await open(nul.store))).toBe("binary");

    const { store } = make({ "a.ts": enc("x") });
    expect(store.beginEdit("tidak-ada")).toBe("notLoaded");
  });

  test("Markdown/JSON hanya bisa diedit pada tampilan mentah; beginEdit beralih ke mentah", async () => {
    const { store } = make({ "r.md": enc("# Judul\n") });
    const id = await open(store, "r.md");
    expect(store.state().tabs[0]?.mode).toBe("render");
    expect(store.beginEdit(id)).toBeNull();
    expect(store.state().tabs[0]?.mode).toBe("raw");
  });

  test("simpan: menulis dengan modified yang diharapkan, memperbarui cache/ukuran/modified, dirty bersih", async () => {
    const { store, disk, writes } = make({ "a.ts": enc("lama") });
    const id = await open(store);
    store.beginEdit(id);
    store.markDirty(id, true);
    expect(store.anyDirty()).toBe(true);
    expect(await store.save(id, "baru\n")).toBe("saved");
    expect(writes).toEqual([{ path: "a.ts", text: "baru\n", expected: 1000 }]);
    expect(new TextDecoder().decode(disk.get("a.ts"))).toBe("baru\n");
    const t = store.state().tabs[0];
    expect(t).toMatchObject({ dirty: false, size: 5, modified: 1001 });
    expect(new TextDecoder().decode(store.content(id) ?? new Uint8Array())).toBe("baru\n");
    expect(store.anyDirty()).toBe(false);
    // simpan lagi memakai modified terbaru
    expect(await store.save(id, "baru2")).toBe("saved");
    expect(writes[1]?.expected).toBe(1001);
  });

  test("konflik: berkas berubah di disk → 'conflict', isi tidak ditimpa, force menimpa", async () => {
    const { store, disk, mtime, writes } = make({ "a.ts": enc("lama") });
    const id = await open(store);
    store.beginEdit(id);
    store.markDirty(id, true);
    mtime.set("a.ts", 5000); // diubah program lain
    disk.set("a.ts", enc("dari-luar"));
    expect(await store.save(id, "punyaku")).toBe("conflict");
    expect(new TextDecoder().decode(disk.get("a.ts"))).toBe("dari-luar");
    expect(store.state().tabs[0]?.dirty).toBe(true);
    expect(await store.save(id, "punyaku", true)).toBe("saved");
    expect(writes.at(-1)?.expected).toBeNull();
    expect(new TextDecoder().decode(disk.get("a.ts"))).toBe("punyaku");
  });

  test("BOM UTF-8 dipertahankan saat menyimpan", async () => {
    const { store, disk } = make({ "a.ts": Uint8Array.from([0xef, 0xbb, 0xbf, ...enc("x")]) });
    const id = await open(store);
    expect(await store.save(id, "y")).toBe("saved");
    expect([...(disk.get("a.ts") ?? [])]).toEqual([0xef, 0xbb, 0xbf, 0x79]);
    expect(store.content(id)).toEqual(Uint8Array.from([0xef, 0xbb, 0xbf, 0x79]));
  });

  test("galat tulis (selain konflik) dilaporkan; tanpa host write → galat", async () => {
    const { store } = make({ "a.ts": enc("x") });
    const id = await open(store);
    const s2 = new DocStore({
      source: {
        read: async () => enc("x"),
        write: async () => {
          throw new Error("disk penuh");
        },
      },
    });
    const id2 = s2.open("C:/p", "b.ts");
    await tick();
    await tick();
    expect(await s2.save(id2, "y")).toEqual({ error: "disk penuh" });
    expect(await store.save("tidak-ada", "y")).toEqual({ error: "penyimpanan tidak tersedia" });
    expect(await make({ "a.ts": enc("x") }, false).store.save(id, "y")).toEqual({
      error: "penyimpanan tidak tersedia",
    });
  });
});
