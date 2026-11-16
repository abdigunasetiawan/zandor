import { describe, expect, it } from "vitest";
import { isLongPaste, isPasted, lineCount, PASTE_MAX_LINES, pastedAttachment, pastedText } from "./pasted-text";

describe("teks ditempel", () => {
  it("mendeteksi teks panjang dari jumlah baris atau karakter", () => {
    expect(isLongPaste("pendek")).toBe(false);
    expect(
      isLongPaste(
        Array(PASTE_MAX_LINES + 1)
          .fill("a")
          .join("\n"),
      ),
    ).toBe(true);
    expect(isLongPaste("x".repeat(2001))).toBe(true);
    expect(lineCount("a\r\nb\nc")).toBe(3);
  });

  it("menyimpan teks UTF-8 utuh dan memberi nama unik", () => {
    const text = "Halo 🌏 dunia\nbaris dua — ünïcödé";
    const first = pastedAttachment(text, []);
    expect(first.filename).toBe("teks-ditempel-1.txt");
    expect(isPasted(first)).toBe(true);
    expect(pastedText(first)).toBe(text);
    expect(pastedAttachment("lagi", [first]).filename).toBe("teks-ditempel-2.txt");
  });

  it("bukan lampiran ditempel → null", () => {
    expect(pastedText({ mime: "text/plain", url: "file:///a.txt", filename: "a.txt" })).toBeNull();
  });
});
