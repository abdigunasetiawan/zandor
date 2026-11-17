import { describe, expect, test } from "vitest";
import { dropHunk, formatHunks, type Hunk, hunkSides, locateHunk, parsePatch, reverseApply, revertHunk } from "./patch";

const BEFORE = ["satu", "dua", "tiga", "empat", "lima", "enam", "tujuh", "delapan", "sembilan", "sepuluh"].join("\n");
const AFTER = [
  "satu",
  "DUA",
  "tiga",
  "empat",
  "lima",
  "enam",
  "tujuh",
  "delapan",
  "sembilan",
  "sepuluh",
  "sebelas",
].join("\n");
// format jsdiff createTwoFilesPatch (seperti engine opencode)
const PATCH = `Index: a.txt
===================================================================
--- a.txt
+++ a.txt
@@ -1,3 +1,3 @@
 satu
-dua
+DUA
 tiga
@@ -8,3 +8,4 @@
 delapan
 sembilan
 sepuluh
+sebelas
\\ No newline at end of file
`;

describe("patch", () => {
  test("parsePatch membaca header dan baris hunk, mengabaikan header berkas", () => {
    const h = parsePatch(PATCH);
    expect(h).toHaveLength(2);
    expect(h[0]).toMatchObject({ oldStart: 1, oldLines: 3, newStart: 1, newLines: 3 });
    expect(h[0]?.lines).toEqual([" satu", "-dua", "+DUA", " tiga"]);
    expect(h[1]?.lines.at(-1)).toBe("+sebelas");
    expect(hunkSides(h[0] as never)).toEqual({ before: ["satu", "dua", "tiga"], after: ["satu", "DUA", "tiga"] });
    expect(parsePatch("")).toEqual([]);
    expect(parsePatch("@@ -1 +1 @@\n a\n")).toEqual([]); // hunk tanpa perubahan dibuang
  });

  test("reverseApply memulihkan isi sebelum; revertHunk hanya satu bagian", () => {
    const h = parsePatch(PATCH);
    expect(reverseApply(AFTER, h)).toBe(BEFORE);
    expect(revertHunk(AFTER, h[0] as never)).toBe(AFTER.replace("DUA", "dua"));
    expect(revertHunk(AFTER, h[1] as never)).toBe(BEFORE.replace("dua", "DUA"));
  });

  test("CRLF dipertahankan dan berkas yang bergeser tetap ditemukan", () => {
    const h = parsePatch(PATCH);
    const crlf = AFTER.replace(/\n/g, "\r\n");
    expect(reverseApply(crlf, h)).toBe(BEFORE.replace(/\n/g, "\r\n"));
    const shifted = `baru 1\nbaru 2\n${AFTER}`;
    expect(locateHunk(shifted, h[0] as never)).toEqual({ line: 3, count: 3 });
    expect(revertHunk(shifted, h[0] as never)).toBe(`baru 1\nbaru 2\n${AFTER.replace("DUA", "dua")}`);
  });

  test("isi yang sudah berubah atau ambigu tidak ditebak", () => {
    const h = parsePatch(PATCH);
    expect(revertHunk(AFTER.replace("DUA", "Dua"), h[0] as never)).toBeNull();
    expect(reverseApply("lain sama sekali", h)).toBeNull();
    const dup = parsePatch("@@ -1,1 +1,1 @@\n-x\n+y\n");
    // "y" muncul dua kali jauh dari posisi yang diharapkan → ditolak
    const far = [...Array(60).fill("z"), "y", ...Array(60).fill("z"), "y"].join("\n");
    expect(revertHunk(far, { ...(dup[0] as Hunk), newStart: 1 })).toBeNull();
  });

  test("dropHunk menggeser hunk sesudahnya bila dikembalikan", () => {
    const h = parsePatch("@@ -1,2 +1,1 @@\n-a\n-b\n+c\n@@ -10,1 +9,1 @@\n-x\n+y\n");
    expect(dropHunk(h, 0, true)[0]?.newStart).toBe(10);
    expect(dropHunk(h, 0, false)[0]?.newStart).toBe(9);
    expect(dropHunk(h, 5, true)).toBe(h);
    expect(parsePatch(formatHunks(h))).toEqual(h);
  });
});
