import { describe, expect, it } from "vitest";
import { isValidLogo, MAX_LOGO_CHARS, ModelPrefs } from "./prefs";

const mem = () => {
  const m = new Map<string, string>();
  return { get: (k: string) => m.get(k) ?? null, set: (k: string, v: string) => void m.set(k, v), m };
};
const PNG = "data:image/png;base64,iVBORw0KGgo=";

describe("ModelPrefs", () => {
  it("menyimpan dan memuat ulang model bawaan, tersembunyi, dan logo", () => {
    const s = mem();
    const p = new ModelPrefs(s);
    p.setDefaultChat({ providerID: "a", modelID: "m1" });
    p.setDefaultImage({ providerID: "b", modelID: "img" });
    p.setVisible([{ providerID: "a", modelID: "m2" }], false);
    expect(p.setProviderLogo("a", PNG)).toBe(true);
    expect(p.setModelLogo({ providerID: "a", modelID: "m1" }, PNG)).toBe(true);
    const again = new ModelPrefs(s).state();
    expect(again.defaultChat).toEqual({ providerID: "a", modelID: "m1" });
    expect(again.defaultImage?.modelID).toBe("img");
    expect(again.hidden).toEqual(["a/m2"]);
    expect(again.providerLogos.a).toBe(PNG);
    expect(again.modelLogos["a/m1"]).toBe(PNG);
  });

  it("pilih semua / batalkan semua bekerja atas himpunan model", () => {
    const p = new ModelPrefs(mem());
    const refs = [1, 2, 3].map((i) => ({ providerID: "a", modelID: `m${i}` }));
    p.setVisible(refs, false);
    expect(p.state().hidden).toHaveLength(3);
    p.setVisible(refs.slice(0, 2), true);
    expect(p.state().hidden).toEqual(["a/m3"]);
    expect(p.isHidden(refs[2] as never)).toBe(true);
    p.setVisible(refs, false); // tidak menggandakan kunci
    expect(p.state().hidden).toHaveLength(3);
  });

  it("menolak logo yang bukan data URL gambar atau terlalu besar", () => {
    expect(isValidLogo(PNG)).toBe(true);
    expect(isValidLogo("https://evil.example/x.png")).toBe(false);
    expect(isValidLogo("data:text/html;base64,PGI+")).toBe(false);
    expect(isValidLogo(`data:image/png;base64,${"A".repeat(MAX_LOGO_CHARS)}`)).toBe(false);
    const p = new ModelPrefs(mem());
    expect(p.setProviderLogo("a", "javascript:alert(1)")).toBe(false);
    expect(p.state().providerLogos).toEqual({});
    p.setProviderLogo("a", PNG);
    p.setProviderLogo("a", null);
    expect(p.state().providerLogos).toEqual({});
  });

  it("data tersimpan yang rusak diabaikan", () => {
    const s = mem();
    s.set("zandor.modelPrefs", "{bukan json");
    expect(new ModelPrefs(s).state().hidden).toEqual([]);
    s.set(
      "zandor.modelPrefs",
      JSON.stringify({ defaultChat: { providerID: 1 }, providerLogos: { a: "http://x/y.png", b: PNG } }),
    );
    const st = new ModelPrefs(s).state();
    expect(st.defaultChat).toBeNull();
    expect(st.providerLogos).toEqual({ b: PNG });
  });
});
