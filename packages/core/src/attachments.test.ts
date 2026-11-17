import { describe, expect, test } from "vitest";
import {
  attachmentFromPath,
  attachmentParts,
  attachmentsFromNote,
  baseNameOf,
  DIRECTORY_MIME,
  fileUrlFromPath,
  mimeForName,
  modelReadable,
  pathFromFileUrl,
} from "./attachments";

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

describe("lampiran yang tidak diterima model", () => {
  test("DOCX dari disk → catatan path sintetis; gambar/PDF/teks tetap bagian file", () => {
    const parts = attachmentParts([
      attachmentFromPath("C:/Data/Tugas BI.docx"),
      attachmentFromPath("C:Data\foto.png"),
      attachmentFromPath("C:/Data/a.pdf"),
      { mime: "application/zip", url: "data:application/zip;base64,AA==", filename: "x.zip" },
    ]);
    expect(parts.map((p) => p.type)).toEqual(["file", "file", "text"]);
    const note = parts[2];
    expect(note?.type === "text" && note.synthetic).toBe(true);
    expect(note?.type === "text" && note.text).toContain("- Tugas BI.docx: C:/Data/Tugas BI.docx");
    expect(note?.type === "text" && note.text).toContain("- x.zip: (tidak tersimpan di disk");
    expect(modelReadable("application/vnd.openxmlformats-officedocument.wordprocessingml.document")).toBe(false);
    expect(modelReadable("text/csv")).toBe(true);
    expect(pathFromFileUrl("file:///C:/a%20b/c.txt")).toBe("C:/a b/c.txt");
    expect(note?.type === "text" ? attachmentsFromNote(note.text) : []).toEqual([
      { name: "Tugas BI.docx", path: "C:/Data/Tugas BI.docx" },
      { name: "x.zip", path: null },
    ]);
  });
});
