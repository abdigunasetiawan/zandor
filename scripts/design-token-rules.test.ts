import { describe, expect, test } from "vitest";
import { findViolations } from "./design-token-rules";

const rules = (file: string, text: string) => findViolations(file, text).map((v) => v.rule);

describe("warna", () => {
  test.each(["#fff", "#FFFFFF", "#1f1f1f80", "#abcd"])("hex %s ditolak di CSS biasa", (hex) => {
    expect(rules("a.css", `.x { color: ${hex}; }`).join()).toMatch(/hex/);
  });
  test("rgb()/hsl() ditolak", () => {
    expect(rules("a.css", ".x { background: rgb(0 0 0 / 0.5); }")).toHaveLength(1);
    expect(rules("a.tsx", 'const c = "hsl(10 20% 30%)";')).toHaveLength(1);
  });
  test("tokens.css boleh menulis nilai mentah", () => {
    expect(
      findViolations("packages/ui/src/styles/tokens.css", "--z-color-bg: #121212; --x: 3px; --y: rgb(0 0 0);"),
    ).toEqual([]);
  });
  test("selector id dan anchor bukan warna", () => {
    expect(findViolations("a.css", "#root { margin: 0; }\n#tokens { x: 0; }")).toEqual([]);
    expect(findViolations("a.tsx", 'const h = "#/design-system"; const a = "#atoms";')).toEqual([]);
  });
  test("var(--z-*) dan nilai transparan aman", () => {
    expect(findViolations("a.css", ".x { color: var(--z-color-fg); border-color: transparent; }")).toEqual([]);
  });
});

describe("ukuran", () => {
  test("px ditolak kecuali 1px", () => {
    expect(rules("a.css", ".x { width: 40px; }").join()).toMatch(/40px/);
    expect(rules("a.css", ".x { margin: 0.5px; }").join()).toMatch(/0.5px/);
    expect(rules("a.css", ".x { border: 1px solid var(--z-color-border); margin: -1px; }")).toEqual([]);
  });
  test("rem dan var() aman", () => {
    expect(findViolations("a.css", ".x { width: 2rem; padding: var(--z-space-2); }")).toEqual([]);
  });
  test("px di string TSX ikut ditolak", () => {
    expect(rules("a.tsx", 'const s = { width: "24px" };')).toHaveLength(1);
  });
});

describe("dialog bawaan OS", () => {
  test.each(['window.confirm("x")', 'alert("x")', "prompt('x')", 'confirm("Hapus?")', "globalThis.alert(1)"])(
    "%s ditolak",
    (code) => {
      expect(rules("a.tsx", `const r = ${code};`).join()).toMatch(/dialog bawaan/);
    },
  );
  test("useConfirm dan confirm({...}) hasil hook aman", () => {
    expect(findViolations("a.tsx", "const confirm = useConfirm(); await confirm({ title: 'x' });")).toEqual([]);
    expect(findViolations("a.tsx", "onConfirm(true); settle(false);")).toEqual([]);
  });
});

describe("komentar", () => {
  test("contoh di komentar tidak dihitung", () => {
    expect(
      findViolations("a.css", "/* contoh: color: #fff; width: 40px; */\n.x { color: var(--z-color-fg); }"),
    ).toEqual([]);
    expect(findViolations("a.ts", "// confirm('x') dan #fff dan 12px\nconst a = 1;")).toEqual([]);
  });
  test("nomor baris tepat", () => {
    const v = findViolations("a.css", "/* a\nb */\n.x {\n  color: #fff;\n}");
    expect(v.map((x) => x.line)).toEqual([4]);
  });
});
