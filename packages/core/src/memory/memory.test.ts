import { describe, expect, test } from "vitest";
import { projectKey as pluginKey } from "../../../plugin/src/keys";
import { memoryEnabled as pluginEnabled, skillCreatorEnabled as pluginSkillEnabled } from "../../../plugin/src/memory";
import { scanSecrets as pluginScan } from "../../../plugin/src/secrets";
import {
  skillMarkdown as pluginSkillMd,
  validateDraft as pluginValidate,
  readQueue,
  writeQueue,
} from "../../../plugin/src/skills";
import { createFakeMemoryHost } from "./fake-host";
import {
  FILE_QUEUE,
  FILE_SETTINGS,
  MemoryStore,
  memoryEnabled,
  parseQueue,
  parseSettings,
  parseSkillMarkdown,
  pinFile,
  projectFile,
  projectKey,
  scanSecrets,
  serializeQueue,
  skillCreatorEnabled,
  skillMarkdown,
  validateSkillDraft,
} from "./store";

const GOOD = {
  name: "tulis-changelog",
  description: "Dipakai saat membuat catatan rilis dari git log.",
  body: "1. Baca git log sejak tag terakhir.\n2. Kelompokkan per tipe commit.",
};
const pending = (id: string, extra: object = {}) => ({
  id,
  t: 1,
  status: "pending" as const,
  trigger: "correction" as const,
  ...GOOD,
  ...extra,
});

describe("paritas dengan plugin (kontrak berkas)", () => {
  test("projectKey sama persis", async () => {
    for (const p of ["C:\\Work\\Alpha", "c:/work/alpha/", "\\\\?\\C:\\Work\\Alpha", "/home/u/proj", "D:/Zandor App"])
      expect(await projectKey(p), p).toBe(pluginKey(p));
  });
  test("pemindai secret dan validasi draf sama", () => {
    const samples = [
      "sk-CONTOHPALSU0123456789abcdefghij",
      "Authorization: Bearer abcdefghijklmnop12345",
      "postgres://admin:hunter22@db/x",
      "MY_API_KEY=abcdef123456789",
      "teks biasa tentang Tauri dan Bun",
    ];
    for (const s of samples) expect(scanSecrets(s), s).toEqual(pluginScan(s));
    for (const d of [GOOD, { ...GOOD, name: "Salah Nama" }, { ...GOOD, body: `${GOOD.body}\n${samples[0]}` }])
      expect(validateSkillDraft(d) === null).toBe(pluginValidate(d) === null);
  });
  test("SKILL.md identik; antrean bolak-balik", () => {
    expect(skillMarkdown(GOOD)).toBe(pluginSkillMd(GOOD));
    expect(parseSkillMarkdown(skillMarkdown(GOOD))).toEqual({ ...GOOD, body: GOOD.body.trim() });
    const q = [pending("a"), pending("b", { status: "approved" })];
    expect(parseQueue(serializeQueue(q))).toEqual(q);
    expect(readQueue).toBeTypeOf("function");
    expect(writeQueue).toBeTypeOf("function");
  });
  test("resolusi opt-in sama (sesi → project → bawaan)", () => {
    const base = parseSettings("");
    const cases = [
      { ...base },
      { ...base, defaultEnabled: true },
      { ...base, projects: { k: true } },
      { ...base, projects: { k: true }, sessions: { s: false } },
      { ...base, defaultEnabled: true, projects: { k: false } },
    ];
    for (const c of cases) {
      expect(memoryEnabled(c, "k", "s")).toBe(pluginEnabled(c, "k", "s"));
      expect(memoryEnabled(c, "k")).toBe(pluginEnabled(c, "k"));
    }
  });
  test("nama berkas sematan aman", () => {
    expect(pinFile("ses_a/b")).toBe("pins/ses_a_b.md");
    expect(projectFile("0123456789abcdef")).toBe("projects/0123456789abcdef.md");
  });
});

describe("MemoryStore", () => {
  const make = () => {
    const host = createFakeMemoryHost();
    return { host, store: new MemoryStore(host) };
  };

  test("refresh: bawaan opt-in mati, antrean kosong; meta memetakan key → path", async () => {
    const { host, store } = make();
    host.files.set("meta.json", JSON.stringify({ projects: { "0123456789abcdef": { path: "C:/Work/a", seen: 1 } } }));
    await store.refresh([]);
    const s = store.state();
    expect(s.ready && !s.loading && s.error === null).toBe(true);
    expect(s.settings.defaultEnabled).toBe(false);
    expect(s.known["0123456789abcdef"]).toBe("C:/Work/a");
    expect(s.queue).toEqual([]);
  });

  test("toggle composer: sesi project berbagi pengaturan project; sesi Chat per sesi; resolusi sama dengan plugin", async () => {
    const { store } = make();
    await store.refresh([]);
    await store.setSessionEnabled("s1", false); // sisa override lama di sesi project
    await store.setScoped("memory", true, "s1", "k"); // sesi dalam project → pengaturan project, override sesi dibuang
    await store.setScoped("skill", true, "s1", "k");
    let s = store.state().settings;
    expect(s.projects.k).toBe(true);
    expect(s.sessions.s1).toBeUndefined();
    expect(s.skillCreatorProjects.k).toBe(true);
    await store.setScoped("memory", true, "chat1", null); // sesi Chat → hanya sesi itu
    await store.setScoped("skill", true, "chat1", null);
    s = store.state().settings;
    expect(s.sessions.chat1).toBe(true);
    expect(s.skillCreatorSessions.chat1).toBe(true);
    expect(s.defaultEnabled).toBe(false);
    expect(s.skillCreator).toBe(false);
    for (const [key, sid] of [
      ["k", "s2"],
      [null, "chat1"],
      [null, "chat2"],
      ["k", "chat1"],
    ] as const) {
      expect(skillCreatorEnabled(s, key, sid)).toBe(pluginSkillEnabled(s, key ?? "zz", sid));
    }
    expect(skillCreatorEnabled(s, "k", "s2")).toBe(true);
    expect(skillCreatorEnabled(s, null, "chat2")).toBe(false);
  });

  test("pengaturan: setiap perubahan membaca ulang dan tidak menimpa perubahan lain", async () => {
    const { host, store } = make();
    await store.refresh([]);
    await store.setDefaultEnabled(true);
    // plugin/UI lain mengubah berkas di antara dua pembaruan
    host.files.set(FILE_SETTINGS, JSON.stringify({ defaultEnabled: true, projects: { k: true }, skillCreator: false }));
    await store.setSkillCreator(true);
    expect(parseSettings(host.files.get(FILE_SETTINGS) ?? "")).toEqual({
      defaultEnabled: true,
      projects: { k: true },
      sessions: {},
      skillCreator: true,
      skillCreatorProjects: {},
      skillCreatorSessions: {},
    });
    await store.setProjectEnabled("k", null);
    await store.setSessionEnabled("s1", false);
    expect(store.state().settings.projects).toEqual({});
    expect(store.state().settings.sessions).toEqual({ s1: false });
  });

  test("berkas memori: simpan, daftar, hapus; berkas tak dikenal ditolak", async () => {
    const { store } = make();
    await store.saveFile("global.md", "- halo\n");
    expect(store.state().files.map((f) => f.rel)).toEqual(["global.md"]);
    expect(await store.readFile("global.md")).toBe("- halo\n");
    await store.deleteFile("global.md");
    expect(store.state().files).toEqual([]);
    await expect(store.saveFile("../x.md", "x")).rejects.toThrow(/tidak dikenal/);
  });

  test("setujui draf: SKILL.md ditulis, status approved, daftar terpasang diperbarui", async () => {
    const { host, store } = make();
    host.files.set(FILE_QUEUE, serializeQueue([pending("a")]));
    await store.refresh(["C:/Work/a"]);
    expect(await store.approve("a", "project", "C:/Work/a")).toBeNull();
    expect(store.state().queue[0]?.status).toBe("approved");
    expect(store.state().installed.projects["C:/Work/a"]?.map((s) => s.name)).toEqual(["tulis-changelog"]);
    expect(await store.skillRead("project", "C:/Work/a", "tulis-changelog")).toBe(skillMarkdown(GOOD));
    // setujui lagi / nama sudah ada
    host.files.set(FILE_QUEUE, serializeQueue([pending("b")]));
    await store.refresh();
    expect(await store.approve("b", "project", "C:/Work/a")).toMatch(/sudah ada/);
    expect(store.state().queue[0]?.status).toBe("pending");
    expect(await store.approve("b", "global", null)).toBeNull();
    expect(store.state().installed.global.map((s) => s.name)).toEqual(["tulis-changelog"]);
  });

  test("setujui menolak: project belum dipilih, draf berisi secret, entri bukan pending", async () => {
    const { host, store } = make();
    host.files.set(
      FILE_QUEUE,
      serializeQueue([
        pending("a"),
        pending("c", { body: "pakai Authorization: Bearer abcdefghijklmnop12345 di semua request" }),
        pending("d", { status: "candidate" }),
      ]),
    );
    await store.refresh();
    expect(await store.approve("a", "project", null)).toMatch(/Pilih project/);
    expect(await store.approve("c", "global", null)).toMatch(/secret/);
    expect(await store.approve("d", "global", null)).toMatch(/belum siap/);
    expect(await store.approve("zzz", "global", null)).toMatch(/tidak ditemukan/);
    expect(host.skills.size).toBe(0);
  });

  test("edit draf divalidasi; tolak dan hapus entri", async () => {
    const { host, store } = make();
    host.files.set(FILE_QUEUE, serializeQueue([pending("a")]));
    await store.refresh();
    expect(await store.editDraft("a", { ...GOOD, name: "Nama Salah" })).toMatch(/Nama skill/);
    expect(await store.editDraft("a", { ...GOOD, name: "nama-baru" })).toBeNull();
    expect(store.state().queue[0]?.name).toBe("nama-baru");
    await store.reject("a");
    expect(store.state().queue[0]?.status).toBe("rejected");
    await store.removeEntry("a");
    expect(store.state().queue).toEqual([]);
  });

  test("approve membaca ulang antrean: baris baru dari plugin tidak hilang", async () => {
    const { host, store } = make();
    host.files.set(FILE_QUEUE, serializeQueue([pending("a")]));
    await store.refresh();
    host.files.set(FILE_QUEUE, serializeQueue([pending("a"), pending("baru", { status: "candidate" })]));
    expect(await store.approve("a", "global", null)).toBeNull();
    expect(parseQueue(host.files.get(FILE_QUEUE) ?? "").map((e) => e.id)).toEqual(["a", "baru"]);
  });

  test("skill terpasang: simpan tervalidasi, nama frontmatter harus sama, hapus", async () => {
    const { host, store } = make();
    await host.skillWrite("global", null, "tulis-changelog", skillMarkdown(GOOD), false);
    await store.refresh();
    const edited = skillMarkdown({ ...GOOD, body: `${GOOD.body}\n3. Tulis ke CHANGELOG.md.` });
    expect(await store.skillSave("global", null, "tulis-changelog", edited)).toBeNull();
    expect(await store.skillRead("global", null, "tulis-changelog")).toBe(edited);
    expect(await store.skillSave("global", null, "tulis-changelog", skillMarkdown({ ...GOOD, name: "lain" }))).toMatch(
      /sama dengan nama folder/,
    );
    expect(
      await store.skillSave(
        "global",
        null,
        "tulis-changelog",
        "teks bebas Authorization: Bearer abcdefghijklmnop12345",
      ),
    ).toMatch(/secret/);
    expect(await store.skillSave("global", null, "tulis-changelog", "teks bebas tanpa frontmatter")).toBeNull();
    await store.skillRemove("global", null, "tulis-changelog");
    expect(store.state().installed.global).toEqual([]);
  });

  test("skill disetujui/diubah/dihapus memberi tahu pengamat agar engine memuat ulang cache skill", async () => {
    const host = createFakeMemoryHost();
    const seen: Array<[string, string | null]> = [];
    const store = new MemoryStore(host, { onSkillsChanged: (scope, path) => seen.push([scope, path]) });
    host.files.set(FILE_QUEUE, serializeQueue([pending("a"), pending("b", { name: "skill-lain" })]));
    await store.refresh(["C:/Work/a"]);
    expect(await store.approve("a", "project", "C:/Work/a")).toBeNull();
    expect(seen).toEqual([["project", "C:/Work/a"]]);
    expect(await store.approve("b", "global", null)).toBeNull();
    expect(seen.at(-1)).toEqual(["global", null]);
    expect(await store.skillSave("global", null, "skill-lain", "teks bebas tanpa frontmatter")).toBeNull();
    await store.skillRemove("global", null, "skill-lain");
    expect(seen).toHaveLength(4);
    // gagal (tanpa project / bukan pending) tidak memberi tahu
    const n = seen.length;
    expect(await store.approve("a", "project", null)).not.toBeNull();
    expect(seen).toHaveLength(n);
  });

  test("pengamat yang melempar tidak menggagalkan persetujuan", async () => {
    const host = createFakeMemoryHost();
    const store = new MemoryStore(host, {
      onSkillsChanged: () => {
        throw new Error("boom");
      },
    });
    host.files.set(FILE_QUEUE, serializeQueue([pending("a")]));
    await store.refresh();
    expect(await store.approve("a", "global", null)).toBeNull();
  });

  test("galat host dilaporkan di state, bukan dilempar", async () => {
    const host = createFakeMemoryHost();
    host.list = async () => {
      throw new Error("disk penuh");
    };
    const store = new MemoryStore(host);
    await store.refresh();
    expect(store.state().error).toBe("disk penuh");
    expect(store.state().loading).toBe(false);
  });
});
