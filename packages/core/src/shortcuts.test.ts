import { describe, expect, test, vi } from "vitest";
import {
  comboFromEvent,
  comboId,
  formatCombo,
  GLOBAL_SCOPE,
  MODAL_SCOPE,
  parseCombo,
  type ShortcutBinding,
  ShortcutRegistry,
} from "./shortcuts";

const ev = (
  key: string,
  mods: Partial<{ ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }> = {},
) => ({
  key,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

const binding = (over: Partial<ShortcutBinding> & Pick<ShortcutBinding, "id" | "combo">): ShortcutBinding => ({
  scope: GLOBAL_SCOPE,
  description: "test",
  run: () => {},
  ...over,
});

describe("parseCombo / formatCombo", () => {
  test("Mod = Ctrl, urutan modifier kanonik, huruf besar", () => {
    expect(formatCombo("shift+mod+k")).toBe("Ctrl+Shift+K");
    expect(comboId(parseCombo("Mod+Shift+K"))).toBe("ctrl+shift+k");
  });

  test("alias tombol", () => {
    expect(parseCombo("Esc").key).toBe("Escape");
    expect(parseCombo("Ctrl+Space").key).toBe(" ");
    expect(formatCombo("Ctrl+Space")).toBe("Ctrl+Space");
  });

  test("Ctrl++ dan Ctrl+- (zoom)", () => {
    expect(parseCombo("Ctrl++").key).toBe("+");
    expect(parseCombo("Ctrl+Plus").key).toBe("+");
    expect(parseCombo("Ctrl+-").key).toBe("-");
    expect(formatCombo("Ctrl++")).toBe("Ctrl+Plus");
  });

  test.each(["", "Ctrl", "Ctrl+", "Ctrl+A+B", "+K"])("menolak combo tidak valid: %j", (bad) => {
    expect(() => parseCombo(bad)).toThrow();
  });

  test("combo dari event cocok dengan combo teks", () => {
    expect(comboId(comboFromEvent(ev("K", { ctrlKey: true, shiftKey: true })))).toBe(
      comboId(parseCombo("Ctrl+Shift+K")),
    );
    expect(comboId(comboFromEvent(ev("Escape")))).toBe("escape");
  });
});

describe("ShortcutRegistry", () => {
  test("menjalankan binding global", () => {
    const run = vi.fn();
    const r = new ShortcutRegistry();
    r.register(binding({ id: "a", combo: "Ctrl+K", run }));
    expect(r.dispatch(ev("k", { ctrlKey: true }), { scopes: [], inInput: false })).toBe(true);
    expect(run).toHaveBeenCalledOnce();
    expect(r.dispatch(ev("j", { ctrlKey: true }), { scopes: [], inInput: false })).toBe(false);
  });

  test("scope terdalam menang atas scope luar dan global", () => {
    const calls: string[] = [];
    const r = new ShortcutRegistry();
    r.register(binding({ id: "g", combo: "Ctrl+E", run: () => calls.push("global") }));
    r.register(binding({ id: "p", combo: "Ctrl+E", scope: "panel.chat", run: () => calls.push("chat") }));
    r.register(binding({ id: "v", combo: "Ctrl+E", scope: "view.settings", run: () => calls.push("settings") }));
    r.dispatch(ev("e", { ctrlKey: true }), { scopes: ["view.settings", "panel.chat"], inInput: false });
    r.dispatch(ev("e", { ctrlKey: true }), { scopes: ["panel.chat"], inInput: false });
    r.dispatch(ev("e", { ctrlKey: true }), { scopes: [], inInput: false });
    expect(calls).toEqual(["settings", "chat", "global"]);
  });

  test("scope lain tidak ikut aktif", () => {
    const run = vi.fn();
    const r = new ShortcutRegistry();
    r.register(binding({ id: "a", combo: "Ctrl+E", scope: "panel.chat", run }));
    expect(r.dispatch(ev("e", { ctrlKey: true }), { scopes: ["panel.docs"], inInput: false })).toBe(false);
    expect(run).not.toHaveBeenCalled();
  });

  test("bentrok: combo sama di scope sama ditolak, di scope beda boleh", () => {
    const r = new ShortcutRegistry();
    r.register(binding({ id: "a", combo: "Ctrl+B" }));
    expect(() => r.register(binding({ id: "b", combo: "Mod+b" }))).toThrow(/bentrok/);
    expect(() => r.register(binding({ id: "c", combo: "Ctrl+B", scope: "panel.chat" }))).not.toThrow();
    expect(() => r.register(binding({ id: "a", combo: "Ctrl+Q" }))).toThrow(/ganda/);
  });

  test("dispose melepas binding sehingga combo bisa dipakai lagi", () => {
    const r = new ShortcutRegistry();
    const off = r.register(binding({ id: "a", combo: "Ctrl+B" }));
    off();
    expect(r.list()).toHaveLength(0);
    expect(() => r.register(binding({ id: "a2", combo: "Ctrl+B" }))).not.toThrow();
  });

  test("scope modal memblokir binding luar kecuali allowInModal", () => {
    const calls: string[] = [];
    const r = new ShortcutRegistry();
    r.register(binding({ id: "g", combo: "Ctrl+B", run: () => calls.push("global") }));
    r.register(binding({ id: "ga", combo: "Ctrl+J", allowInModal: true, run: () => calls.push("global-ok") }));
    r.register(binding({ id: "m", combo: "Ctrl+B", scope: MODAL_SCOPE, run: () => calls.push("modal") }));
    const ctx = { scopes: [MODAL_SCOPE, "panel.chat"], inInput: false };
    r.dispatch(ev("b", { ctrlKey: true }), ctx);
    r.dispatch(ev("j", { ctrlKey: true }), ctx);
    expect(calls).toEqual(["modal", "global-ok"]);
    expect(r.dispatch(ev("b", { ctrlKey: true }), { scopes: ["panel.chat"], inInput: false })).toBe(true);
    expect(calls.at(-1)).toBe("global");
  });

  test("saat mengetik: tanpa modifier diabaikan kecuali allowInInput; dengan Ctrl tetap jalan", () => {
    const run = vi.fn();
    const r = new ShortcutRegistry();
    r.register(binding({ id: "slash", combo: "/", run }));
    r.register(binding({ id: "esc", combo: "Escape", allowInInput: true, run }));
    r.register(binding({ id: "k", combo: "Ctrl+K", run }));
    const typing = { scopes: [], inInput: true };
    expect(r.dispatch(ev("/"), typing)).toBe(false);
    expect(r.dispatch(ev("Escape"), typing)).toBe(true);
    expect(r.dispatch(ev("k", { ctrlKey: true }), typing)).toBe(true);
    expect(r.dispatch(ev("/"), { scopes: [], inInput: false })).toBe(true);
  });

  test("when=false melewati binding dan mencoba berikutnya", () => {
    const calls: string[] = [];
    const r = new ShortcutRegistry();
    r.register(binding({ id: "g", combo: "Ctrl+E", run: () => calls.push("global") }));
    r.register(
      binding({ id: "p", combo: "Ctrl+E", scope: "panel.chat", when: () => false, run: () => calls.push("chat") }),
    );
    r.dispatch(ev("e", { ctrlKey: true }), { scopes: ["panel.chat"], inInput: false });
    expect(calls).toEqual(["global"]);
  });

  test("subscribe: dipanggil saat daftar berubah; snapshot stabil bila tidak berubah", () => {
    const r = new ShortcutRegistry();
    const listener = vi.fn();
    const off = r.subscribe(listener);
    const empty = r.list();
    expect(r.list()).toBe(empty);
    const dispose = r.register(binding({ id: "a", combo: "Ctrl+B" }));
    expect(listener).toHaveBeenCalledTimes(1);
    const one = r.list();
    expect(one).not.toBe(empty);
    expect(r.list()).toBe(one);
    dispose();
    dispose(); // idempoten: tidak memicu lagi
    expect(listener).toHaveBeenCalledTimes(2);
    off();
    r.register(binding({ id: "b", combo: "Ctrl+Q" }));
    expect(listener).toHaveBeenCalledTimes(2);
  });
});
