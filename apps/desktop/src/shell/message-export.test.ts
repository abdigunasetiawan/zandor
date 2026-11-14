import { describe, expect, test } from "vitest";
import { dataUrlToBytes, exportFileName, textBytes } from "./message-export";

describe("ekspor respons", () => {
  test("nama berkas berstempel waktu dan berekstensi sesuai jenis", () => {
    const at = new Date(2026, 10, 14, 9, 5, 7);
    expect(exportFileName("text", at)).toBe("respons-20261114-090507.md");
    expect(exportFileName("png", at)).toBe("respons-20261114-090507.png");
    expect(exportFileName("svg", at)).toBe("respons-20261114-090507.svg");
  });
  test("teks diakhiri satu baris baru dan disandikan UTF-8", () => {
    expect(new TextDecoder().decode(textBytes("halo ✓"))).toBe("halo ✓\n");
    expect(new TextDecoder().decode(textBytes("sudah\n"))).toBe("sudah\n");
  });
  test("dekode data URL base64 dan persen-encoded; yang tidak valid ditolak", () => {
    expect(Array.from(dataUrlToBytes("data:image/png;base64,AQID"))).toEqual([1, 2, 3]);
    expect(new TextDecoder().decode(dataUrlToBytes("data:image/svg+xml;charset=utf-8,%3Csvg%2F%3E"))).toBe("<svg/>");
    expect(() => dataUrlToBytes("https://x")).toThrow();
    expect(() => dataUrlToBytes("data:text/plain")).toThrow();
  });
});
