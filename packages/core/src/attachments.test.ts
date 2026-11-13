import { describe, expect, test } from "vitest";
import { attachmentFromPath, baseNameOf, DIRECTORY_MIME, fileUrlFromPath, mimeForName } from "./attachments";

describe("lampiran", () => {
  test("URL file dari path Windows (spasi, unicode, awalan verbatim)", () => {
    expect(fileUrlFromPath("C:\\Users\\A B\\ü.txt")).toBe("file:///C:/Users/A%20B/%C3%BC.txt");
    expect(fileUrlFromPath("\\\\?\\C:\\x\\y.md")).toBe("file:///C:/x/y.md");
    expect(fileUrlFromPath("/home/a b/c")).toBe("file:///home/a%20b/c");
  });
  test("karakter khusus di nama (#, ?, %) di-escape", () => {
    expect(fileUrlFromPath("C:\\a\\b#c?d%e.txt")).toBe("file:///C:/a/b%23c%3Fd%25e.txt");
  });
  test("MIME dari ekstensi; tak dikenal → text/plain; tanpa ekstensi aman", () => {
    expect(mimeForName("a.PNG")).toBe("image/png");
    expect(mimeForName("doc.pdf")).toBe("application/pdf");
    expect(mimeForName("Makefile")).toBe("text/plain");
    expect(mimeForName("x.zzz")).toBe("text/plain");
  });
  test("attachmentFromPath: berkas dan folder", () => {
    expect(attachmentFromPath("C:\\p\\gambar.jpg")).toEqual({
      mime: "image/jpeg",
      url: "file:///C:/p/gambar.jpg",
      filename: "gambar.jpg",
    });
    expect(attachmentFromPath("C:\\p\\src", true).mime).toBe(DIRECTORY_MIME);
    expect(baseNameOf("C:\\p\\src\\")).toBe("src");
  });
});
