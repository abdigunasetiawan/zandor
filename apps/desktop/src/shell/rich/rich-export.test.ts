// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { extForLang, latexOf, stampedName, tableMatrix, toCsv, toMarkdownTable, toTsv } from "./rich-export";

const ROWS = [
  ["Nama", "Nilai"],
  ["a|b", "1,5"],
  ['kata "x"', "2"],
];

describe("rich-export", () => {
  it("tabel → Markdown, TSV, CSV", () => {
    expect(toMarkdownTable(ROWS)).toBe('| Nama | Nilai |\n| --- | --- |\n| a\\|b | 1,5 |\n| kata "x" | 2 |');
    expect(toTsv(ROWS).split("\n")[1]).toBe("a|b\t1,5");
    expect(toCsv(ROWS)).toBe('Nama,Nilai\r\na|b,"1,5"\r\n"kata ""x""",2');
  });

  it("membaca matriks dari elemen tabel", () => {
    document.body.innerHTML =
      "<table><thead><tr><th>A</th><th> B </th></tr></thead><tbody><tr><td>1</td><td>2\n3</td></tr></tbody></table>";
    expect(tableMatrix(document.querySelector("table") as HTMLTableElement)).toEqual([
      ["A", "B"],
      ["1", "2 3"],
    ]);
  });

  it("ekstensi dan nama berkas", () => {
    expect(extForLang("TypeScript")).toBe("ts");
    expect(extForLang(null)).toBe("txt");
    expect(extForLang("brainfuck")).toBe("txt");
    expect(stampedName("kode", "py", new Date(2026, 0, 2, 3, 4, 5))).toBe("kode-20260102-030405.py");
  });

  it("mengambil LaTeX dari anotasi KaTeX", () => {
    const node = {
      tagName: "span",
      children: [
        {
          tagName: "annotation",
          properties: { encoding: "application/x-tex" },
          children: [{ type: "text", value: "x^2" }],
        },
      ],
    };
    expect(latexOf(node)).toBe("x^2");
    expect(latexOf({ tagName: "span" })).toBeNull();
  });
});
