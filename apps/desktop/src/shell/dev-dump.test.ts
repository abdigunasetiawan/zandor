// @vitest-environment jsdom
import { describe, expect, test } from "vitest";
import { buildPanelDump, jsonForScript } from "./dev-dump";

describe("jsonForScript", () => {
  test("tidak pernah menutup <script> lebih awal", () => {
    const out = jsonForScript({ x: "</script><b>" });
    expect(out).not.toContain("</script");
    expect(JSON.parse(out)).toEqual({ x: "</script><b>" });
  });
});

describe("buildPanelDump", () => {
  test("memuat DOM panel, CSS, atribut tema, meta, dan data mentah", () => {
    document.documentElement.setAttribute("data-theme", "dark");
    document.body.className = "kelas-uji";
    const style = document.createElement("style");
    style.textContent = ".msg { color: red; }";
    document.head.append(style);
    const root = document.createElement("section");
    root.setAttribute("aria-label", "Chat");
    root.innerHTML = '<div class="msg">Halo <b>dunia</b></div>';
    document.body.append(root);

    const html = buildPanelDump({
      doc: document,
      root,
      title: "Dump uji",
      meta: { sesi: "s1" },
      data: [{ id: "m1", text: "</script>" }],
    });

    expect(html.startsWith("<!doctype html>")).toBe(true);
    expect(html).toContain('data-theme="dark"');
    expect(html).toContain('class="kelas-uji"');
    expect(html).toContain(".msg");
    expect(html).toContain('<section aria-label="Chat">');
    expect(html).toContain("Halo <b>dunia</b>");
    expect(html).toContain('id="zandor-dump-meta"');
    // data memuat "</script>" tetapi tidak boleh memutus dokumen
    const dataBlock = html.slice(html.indexOf('id="zandor-dump-data"'));
    expect(dataBlock.indexOf("</script>")).toBe(dataBlock.lastIndexOf("</script>"));
    // dokumen hasil dapat dibaca ulang dan data mentah utuh
    const parsed = new DOMParser().parseFromString(html, "text/html");
    expect(JSON.parse(parsed.getElementById("zandor-dump-data")?.textContent ?? "null")).toEqual([
      { id: "m1", text: "</script>" },
    ]);
    expect(parsed.querySelector(".msg")?.textContent).toBe("Halo dunia");
  });
});
