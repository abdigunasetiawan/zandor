import { describe, expect, test } from "vitest";
import { ShortcutRegistry } from "./shortcuts";
import { AppearanceStore, DEFAULT_APPEARANCE, sanitizeAppearance } from "./theme";

const bind = (id: string, combo: string, scope = "global") => ({
  id,
  combo,
  scope,
  description: id,
  run: () => {},
});

describe("override shortcut", () => {
  test("ganti combo, combo lama bebas, bawaan bisa dikembalikan", () => {
    const r = new ShortcutRegistry();
    r.register(bind("a", "Ctrl+K"));
    expect(r.setOverride("a", "Ctrl+Shift+P")).toBeNull();
    expect(r.comboOf("a")).toBe("Ctrl+Shift+P");
    expect(r.defaultComboOf("a")).toBe("Ctrl+K");
    const ctx = { scopes: ["global"], inInput: false };
    const ev = (key: string, shift = false) => ({ key, ctrlKey: true, altKey: false, shiftKey: shift, metaKey: false });
    expect(r.resolve({ key: "p", ctrl: true, alt: false, shift: true, meta: false }, ctx)?.id).toBe("a");
    expect(r.dispatch(ev("k"), ctx)).toBe(false);
    expect(r.setOverride("a", null)).toBeNull();
    expect(r.comboOf("a")).toBe("Ctrl+K");
    expect(r.dispatch(ev("k"), ctx)).toBe(true);
  });

  test("bentrok di scope yang sama ditolak; scope berbeda boleh; combo salah ditolak", () => {
    const r = new ShortcutRegistry();
    r.register(bind("a", "Ctrl+K"));
    r.register(bind("b", "Ctrl+J"));
    r.register(bind("c", "Ctrl+K", "editor"));
    expect(r.setOverride("b", "Ctrl+K")).toMatch(/bentrok.*a/);
    expect(r.comboOf("b")).toBe("Ctrl+J");
    expect(r.setOverride("c", "Ctrl+J")).toBeNull();
    expect(r.setOverride("a", "Ctrl+")).not.toBeNull();
    expect(r.setOverride("nope", "Ctrl+X")).toMatch(/tidak dikenal/);
  });

  test("override tersimpan berlaku saat binding didaftarkan belakangan; yang bentrok jatuh ke bawaan", () => {
    const r = new ShortcutRegistry();
    r.applyOverrides({ late: "Ctrl+Shift+L", clash: "Ctrl+J" });
    r.register(bind("other", "Ctrl+J"));
    r.register(bind("late", "Ctrl+L"));
    r.register(bind("clash", "Ctrl+M"));
    expect(r.comboOf("late")).toBe("Ctrl+Shift+L");
    expect(r.comboOf("clash")).toBe("Ctrl+M");
  });

  test("applyOverrides menghapus override yang tak ada lagi di peta", () => {
    const r = new ShortcutRegistry();
    r.register(bind("a", "Ctrl+K"));
    expect(r.applyOverrides({ a: "Ctrl+Shift+K" })).toEqual({});
    expect(r.comboOf("a")).toBe("Ctrl+Shift+K");
    r.applyOverrides({});
    expect(r.comboOf("a")).toBe("Ctrl+K");
  });

  test("daftar berubah → pelanggan diberi tahu", () => {
    const r = new ShortcutRegistry();
    r.register(bind("a", "Ctrl+K"));
    let n = 0;
    r.subscribe(() => n++);
    r.setOverride("a", "Ctrl+Shift+K");
    expect(n).toBe(1);
  });
});

describe("override tersimpan di AppearanceStore", () => {
  test("simpan, muat ulang, buang yang rusak, reset", () => {
    let raw: string | null = null;
    const st = new AppearanceStore({
      get: () => raw,
      set: (v) => {
        raw = v;
      },
    });
    expect(st.setShortcut("view.chat", "Ctrl+Alt+C")).toBeNull();
    expect(st.setShortcut("view.chat", "Ctrl+")).not.toBeNull();
    expect(new AppearanceStore({ get: () => raw, set: () => {} }).state().shortcuts).toEqual({
      "view.chat": "Ctrl+Alt+C",
    });
    expect(
      sanitizeAppearance({ shortcuts: { "cmd:app.ok": "Ctrl+B", bad: "Ctrl+", "bad id!": "Ctrl+C", n: 5 } }).shortcuts,
    ).toEqual({
      "cmd:app.ok": "Ctrl+B",
    });
    st.setShortcut("view.chat", null);
    expect(st.state().shortcuts).toEqual({});
    st.setShortcut("x", "Ctrl+X");
    st.resetAll();
    expect(st.state()).toEqual(DEFAULT_APPEARANCE);
  });
});
