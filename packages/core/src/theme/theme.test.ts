import { describe, expect, test } from "vitest";
import {
  AppearanceStore,
  computeVars,
  DEFAULT_APPEARANCE,
  sanitizeAppearance,
  validateChat,
  validateLogo,
} from "./appearance";
import { PRESETS, presetById } from "./presets";
import { contrastRatio, isColor, sanitizeTheme, validateFontFamily, validateTypeValue } from "./tokens";

const mem = (initial: string | null = null) => {
  let v = initial;
  return {
    get: () => v,
    set: (x: string) => {
      v = x;
    },
    read: () => v,
  };
};

describe("validator nilai", () => {
  test("warna: heks dan fungsi aman; sisanya ditolak", () => {
    for (const ok of ["#fff", "#FFFFFF", "#ffffff80", "rgb(0 0 0)", "rgba(0, 0, 0, 0.5)", "hsl(120 50% 50%)"])
      expect(isColor(ok), ok).toBe(true);
    for (const bad of [
      "red",
      "url(http://x)",
      "#12",
      "#ggg",
      "#fff;} body{display:none",
      "var(--x)",
      "expression(1)",
      "",
      1,
      null,
    ])
      expect(isColor(bad), String(bad)).toBe(false);
  });

  test("tipografi: batas dan satuan", () => {
    expect(validateTypeValue("size", "0.875rem")).toBeNull();
    expect(validateTypeValue("size", "14px")).toMatch(/rem/);
    expect(validateTypeValue("size", "9rem")).toMatch(/0.5rem–4rem/);
    expect(validateTypeValue("weight", "650")).toBeNull();
    expect(validateTypeValue("weight", "1000")).not.toBeNull();
    expect(validateTypeValue("lh", "1.5")).toBeNull();
    expect(validateTypeValue("lh", "0.5")).not.toBeNull();
    expect(validateTypeValue("tracking", "-0.02em")).toBeNull();
    expect(validateTypeValue("tracking", "0")).toBeNull();
    expect(validateTypeValue("tracking", "2em")).not.toBeNull();
  });

  test("font: nama aman saja", () => {
    expect(validateFontFamily('"Plus Jakarta Sans", system-ui, sans-serif')).toBeNull();
    for (const bad of ["", "a; color:red", "x{}", "url(x)", "a(b)", "a".repeat(201)])
      expect(validateFontFamily(bad), bad).not.toBeNull();
  });

  test("panel chat dan logo", () => {
    expect(validateChat("opacity", 0.8)).toBeNull();
    expect(validateChat("opacity", 0.1)).not.toBeNull();
    expect(validateChat("weight", 450)).toBeNull();
    expect(validateChat("weight", 450.5)).not.toBeNull();
    expect(validateChat("size", "0.9rem")).toBeNull();
    expect(validateChat("gap", "5rem")).not.toBeNull();
    expect(validateLogo("🚀")).toBeNull();
    expect(validateLogo("AB")).toBeNull();
    for (const bad of ["", "<b>", "abcde", 'a"b', "a\nb"]) expect(validateLogo(bad), bad).not.toBeNull();
  });
});

describe("preset bawaan", () => {
  test("id unik; ada preset yang diminta roadmap", () => {
    const ids = PRESETS.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const want of ["zandor", "gemini", "claude", "chatgpt", "discord", "vercel", "raycast"])
      expect(ids).toContain(want);
  });

  test("semua nilai warna valid dan tema lolos sanitasi", () => {
    for (const p of PRESETS) {
      for (const m of [p.dark, p.light])
        for (const v of Object.values(m)) expect(isColor(v), `${p.id} ${v}`).toBe(true);
      expect(sanitizeTheme(p).errors, p.id).toEqual([]);
    }
  });

  test("kontras WCAG AA: teks utama/redup/halus di atas latar, teks di atas aksen", () => {
    const defaults = {
      dark: { bg: "#121212", raised: "#1f1f1f", fg: "#ececec", "fg-muted": "#a3a3a3", "fg-subtle": "#8a8a8a" },
      light: { bg: "#fafafa", raised: "#ffffff", fg: "#161616", "fg-muted": "#5c5c5c", "fg-subtle": "#6b6b6b" },
    };
    for (const p of PRESETS)
      for (const mode of ["dark", "light"] as const) {
        const c = {
          ...defaults[mode],
          accent: mode === "dark" ? "#ececec" : "#161616",
          "on-accent": mode === "dark" ? "#121212" : "#fafafa",
          ...p[mode],
        } as Record<string, string>;
        const need = (a: string, b: string, min: number, what: string) => {
          const r = contrastRatio(c[a] as string, c[b] as string) ?? 0;
          expect(r, `${p.id}/${mode} ${what} = ${r.toFixed(2)}`).toBeGreaterThanOrEqual(min);
        };
        need("fg", "bg", 7, "fg/bg");
        need("fg-muted", "bg", 4.5, "fg-muted/bg");
        need("fg-subtle", "bg", 4.5, "fg-subtle/bg");
        need("fg", "raised", 4.5, "fg/raised");
        need("fg-muted", "raised", 4.5, "fg-muted/raised");
        need("on-accent", "accent", 4.5, "on-accent/accent");
        need("accent", "bg", 3, "accent/bg");
      }
  });

  test("contrastRatio: nilai acuan WCAG", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 0);
    expect(contrastRatio("#777777", "#777777")).toBeCloseTo(1, 5);
    expect(contrastRatio("red", "#fff")).toBeNull();
  });
});

describe("sanitizeTheme", () => {
  const good = { id: "kopi", name: "Kopi", dark: { bg: "#111111" } };
  test("tema valid lolos; galat dikumpulkan", () => {
    expect(sanitizeTheme(good).theme).toEqual({ id: "kopi", name: "Kopi", dark: { bg: "#111111" } });
    const r = sanitizeTheme({ id: "Bad Id", name: "", dark: { bg: "red", zzz: "#fff" }, light: [] });
    expect(r.theme).toBeNull();
    expect(r.errors.join("|")).toMatch(/id harus/);
    expect(r.errors.join("|")).toMatch(/name wajib/);
    expect(r.errors.join("|")).toMatch(/dark.bg/);
    expect(r.errors.join("|")).toMatch(/dark.zzz/);
    expect(r.errors.join("|")).toMatch(/light/);
    expect(sanitizeTheme({ id: "a", name: "A" }).errors.join()).toMatch(/dark.*light/);
    expect(sanitizeTheme(null).theme).toBeNull();
    expect(sanitizeTheme([]).theme).toBeNull();
  });
});

describe("computeVars", () => {
  test("default tidak mengubah apa pun", () => {
    expect(computeVars(DEFAULT_APPEARANCE, "dark")).toEqual({});
    expect(computeVars(DEFAULT_APPEARANCE, "light")).toEqual({});
  });
  test("preset memberi warna sesuai mode; tipografi/font/chat memberi variabel", () => {
    const a = {
      ...DEFAULT_APPEARANCE,
      preset: "claude",
      typography: { h1: { size: "2rem", weight: "800" } },
      fonts: { ui: "Inter, sans-serif" },
      chat: { opacity: 0.9, size: "1rem" },
    };
    const dark = computeVars(a, "dark");
    expect(dark["--z-color-bg"]).toBe(presetById("claude")?.dark.bg);
    expect(computeVars(a, "light")["--z-color-bg"]).toBe(presetById("claude")?.light.bg);
    expect(dark["--z-type-h1-size"]).toBe("2rem");
    expect(dark["--z-type-h1-weight"]).toBe("800");
    expect(dark["--z-font-ui"]).toBe("Inter, sans-serif");
    expect(dark["--z-chat-opacity"]).toBe("0.9");
    expect(dark["--z-chat-size"]).toBe("1rem");
  });
  test("nilai tidak valid tidak pernah menjadi variabel CSS", () => {
    const evil = {
      ...DEFAULT_APPEARANCE,
      typography: { h1: { size: "1px;}body{display:none" } },
      fonts: { ui: "a;b" },
      chat: { size: "url(x)" },
    } as never;
    expect(computeVars(evil, "dark")).toEqual({});
  });
});

describe("sanitizeAppearance (data tersimpan rusak)", () => {
  test("bagian rusak dibuang, bagian baik dipertahankan", () => {
    const a = sanitizeAppearance({
      preset: "tidak-ada",
      typography: { h1: { size: "2rem", weight: "99999" }, zz: {} },
      fonts: { ui: "x;y", code: "Consolas" },
      chat: { opacity: 5, weight: 500 },
      projects: { p1: { border: "red", background: "#112233", logo: "🚀" }, p2: { logo: "<b>" } },
      editorTheme: "vs-dark",
      customThemes: [
        { id: "zandor", name: "Palsu", dark: { bg: "#000" } },
        { id: "ok", name: "Ok", dark: { bg: "#000" } },
      ],
    });
    expect(a.preset).toBe("zandor");
    expect(a.typography).toEqual({ h1: { size: "2rem" } });
    expect(a.fonts).toEqual({ code: "Consolas" });
    expect(a.chat).toEqual({ weight: 500 });
    expect(a.projects).toEqual({ p1: { background: "#112233", logo: "🚀" } });
    expect(a.editorTheme).toBe("vs-dark");
    expect(a.customThemes.map((t) => t.id)).toEqual(["ok"]);
    expect(sanitizeAppearance("bukan objek")).toEqual(DEFAULT_APPEARANCE);
  });
});

describe("AppearanceStore", () => {
  test("pilih preset, persisten, dan dimuat ulang", () => {
    const s = mem();
    const st = new AppearanceStore(s);
    st.setPreset("gemini");
    expect(new AppearanceStore(s).state().preset).toBe("gemini");
    expect(() => st.setPreset("tidak-ada")).toThrow(/tidak dikenal/);
  });

  test("tema kustom: simpan, tolak id preset, impor/ekspor bolak-balik, hapus mengembalikan preset", () => {
    const st = new AppearanceStore(mem());
    expect(st.saveCustomTheme({ id: "vercel", name: "x", dark: { bg: "#000" } })?.[0]).toMatch(/dipakai preset/);
    expect(st.saveCustomTheme({ id: "kopi", name: "Kopi", dark: { bg: "#221100", fg: "#ffeedd" } })).toBeNull();
    st.setPreset("kopi");
    expect(computeVars(st.state(), "dark")["--z-color-bg"]).toBe("#221100");
    const json = st.exportTheme("kopi");
    st.removeCustomTheme("kopi");
    expect(st.state().preset).toBe("zandor");
    expect(st.importTheme(json)).toEqual({ ok: true, id: "kopi" });
    expect(st.state().customThemes[0]?.dark?.bg).toBe("#221100");
    // ekspor preset bawaan lalu impor → id diberi akhiran
    const r = st.importTheme(st.exportTheme("discord"));
    expect(r).toEqual({ ok: true, id: "discord-impor" });
    expect(st.themes().map((t) => t.id)).toContain("discord-impor");
  });

  test("impor menolak JSON rusak, warna jahat, dan berkas besar", () => {
    const st = new AppearanceStore(mem());
    expect(st.importTheme("{bukan json")).toEqual({ ok: false, errors: ["bukan JSON yang valid"] });
    const bad = st.importTheme(JSON.stringify({ id: "x", name: "X", dark: { bg: "url(javascript:alert(1))" } }));
    expect(bad.ok).toBe(false);
    expect(
      st.importTheme(JSON.stringify({ id: "x", name: "X", dark: { bg: "#000" }, pad: "a".repeat(100_001) })).ok,
    ).toBe(false);
    expect(st.state().customThemes).toEqual([]);
  });

  test("tipografi, font, chat, project: set, validasi, reset ke default", () => {
    const st = new AppearanceStore(mem());
    expect(st.setTypography("h2", "size", "1.5rem")).toBeNull();
    expect(st.setTypography("h2", "size", "bad")).toMatch(/rem/);
    expect(st.state().typography.h2).toEqual({ size: "1.5rem" });
    expect(st.setTypography("h2", "size", null)).toBeNull();
    expect(st.state().typography.h2).toBeUndefined();
    expect(st.setFont("code", "Consolas")).toBeNull();
    expect(st.setFont("code", "a;b")).not.toBeNull();
    expect(st.setChat("opacity", 0.7)).toBeNull();
    expect(st.setChat("opacity", 9)).not.toBeNull();
    expect(st.setProjectStyle("p1", "logo", "🚀")).toBeNull();
    expect(st.setProjectStyle("p1", "border", "red")).not.toBeNull();
    expect(st.setProjectStyle("p1", "logo", null)).toBeNull();
    expect(st.state().projects.p1).toBeUndefined();
    st.resetAll();
    expect(st.state()).toEqual(DEFAULT_APPEARANCE);
  });

  test("subscribe diberi tahu; penyimpanan rusak → default", () => {
    const st = new AppearanceStore(mem());
    let n = 0;
    const off = st.subscribe(() => n++);
    st.setPreset("claude");
    off();
    st.setPreset("zandor");
    expect(n).toBe(1);
    expect(new AppearanceStore(mem("{rusak")).state()).toEqual(DEFAULT_APPEARANCE);
  });
});

describe("profil pengguna", () => {
  const memStore = () => {
    let v: string | null = null;
    return {
      get: () => v,
      set: (x: string) => {
        v = x;
      },
    };
  };
  test("nama dan avatar ilustrasi tersimpan, bertahan, dan dipertahankan saat atur ulang tampilan", () => {
    const s = memStore();
    const st = new AppearanceStore(s);
    expect(st.setProfileName("  Abdi  ")).toBeNull();
    expect(st.setProfileAvatar({ kind: "illustration", style: "lorelei", seed: "abc-1" })).toBeNull();
    const again = new AppearanceStore(s);
    expect(again.state().profile).toEqual({
      name: "Abdi",
      avatar: { kind: "illustration", style: "lorelei", seed: "abc-1" },
    });
    again.resetAll();
    expect(again.state().profile.name).toBe("Abdi");
    expect(again.setProfileName("")).toBeNull();
    expect(again.state().profile.name).toBeUndefined();
  });
  test("menolak nilai tidak valid dan membuang data rusak saat dimuat", () => {
    const st = new AppearanceStore(memStore());
    expect(st.setProfileName("x".repeat(41))).not.toBeNull();
    expect(st.setProfileAvatar({ kind: "illustration", style: "bogus" as never, seed: "a" })).not.toBeNull();
    expect(st.setProfileAvatar({ kind: "image", src: "javascript:alert(1)" })).not.toBeNull();
    const raw = JSON.stringify({ profile: { name: "\u0001", avatar: { kind: "image", src: "http://x" } } });
    expect(new AppearanceStore({ get: () => raw, set: () => {} }).state().profile).toEqual({});
  });
  test("gaya chat lanjutan: lebar, tinggi baris, jarak huruf, ukuran kode, rasio composer", () => {
    const a = sanitizeAppearance({
      chat: { width: "60rem", lineHeight: 1.8, tracking: "0.02em", codeSize: "0.9rem", composerRatio: 1.4 },
    });
    const v = computeVars(a, "dark");
    expect(v["--z-chat-width"]).toBe("60rem");
    expect(v["--z-chat-line"]).toBe("1.8");
    expect(v["--z-chat-tracking"]).toBe("0.02em");
    expect(v["--z-chat-code-size"]).toBe("0.9rem");
    expect(v["--z-chat-composer-ratio"]).toBe("1.4");
    const bad = sanitizeAppearance({
      chat: { width: "5rem", lineHeight: 9, tracking: "1em", codeSize: "url(x)", composerRatio: 3 },
    });
    expect(bad.chat).toEqual({});
  });
  test("kustomisasi dokumen (Panel 3): validasi, CSS var, store", () => {
    const a = sanitizeAppearance({
      docs: {
        codeSize: 16,
        minimap: true,
        theme: "sepia",
        proseSize: "1rem",
        proseWidth: "50rem",
        tableSize: "0.8rem",
      },
    });
    expect(a.docs).toMatchObject({ codeSize: 16, minimap: true, theme: "sepia" });
    const v = computeVars(a, "light");
    expect(v["--z-doc-size"]).toBe("1rem");
    expect(v["--z-doc-width"]).toBe("50rem");
    expect(v["--z-doc-table-size"]).toBe("0.8rem");
    const bad = sanitizeAppearance({
      docs: { codeSize: 99, minimap: "ya", theme: "neon", proseWidth: "url(x)", imageBg: "x", codeFont: "<x>" },
    });
    expect(bad.docs).toEqual({});
    let raw: string | null = null;
    const st = new AppearanceStore({ get: () => raw, set: (x) => (raw = x) });
    expect(st.setDoc("codeSize", 20)).toBeNull();
    expect(st.setDoc("codeSize", 3)).not.toBeNull();
    expect(st.state().docs.codeSize).toBe(20);
    st.setDoc("theme", "night");
    st.resetDocs(["codeSize"]);
    expect(st.state().docs).toEqual({ theme: "night" });
    st.resetDocs();
    expect(st.state().docs).toEqual({});
  });
});
