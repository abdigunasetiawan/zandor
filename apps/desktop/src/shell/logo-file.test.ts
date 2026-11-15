// @vitest-environment jsdom
import { describe, expect, test } from "vitest";
import { fileToLogo } from "./logo-file";

describe("fileToLogo", () => {
  test("SVG diterima apa adanya sebagai data URL", async () => {
    const svg = new File(["<svg xmlns='http://www.w3.org/2000/svg' width='4' height='4'/>"], "l.svg", {
      type: "image/svg+xml",
    });
    expect(await fileToLogo(svg)).toMatch(/^data:image\/svg\+xml;base64,/);
  });
  test("jenis non-gambar ditolak", async () => {
    await expect(fileToLogo(new File(["x"], "a.txt", { type: "text/plain" }))).rejects.toThrow("type");
    await expect(fileToLogo(new File(["<html>"], "a.html", { type: "text/html" }))).rejects.toThrow("type");
  });
  test("SVG terlalu besar ditolak", async () => {
    const big = new File([`<svg>${"a".repeat(200_000)}</svg>`], "b.svg", { type: "image/svg+xml" });
    await expect(fileToLogo(big)).rejects.toThrow("size");
  });
});
