import { describe, expect, it } from "vitest";
import {
  buildMemory,
  conflictsOf,
  isSingleKey,
  listsOf,
  mergeMemory,
  parseMemoryText,
  unresolved,
  valueId,
} from "./import";

describe("impor memori", () => {
  it("mengurai daftar kunci: nilai dari blok kode dan kalimat bebas", () => {
    const p = parseMemoryText("```\n- nama: Abdi\n* **Pekerjaan**: developer\n1. suka jawaban singkat\n# Judul\n```");
    expect(p).toEqual([
      { key: "nama", label: "nama", value: "Abdi" },
      { key: "pekerjaan", label: "Pekerjaan", value: "developer" },
      { key: null, label: null, value: "suka jawaban singkat" },
    ]);
  });

  it("nilai sama dari banyak AI digabung; nilai berbeda jadi konflik", () => {
    const m = mergeMemory([
      { id: "gemini", label: "Gemini", text: "- nama: Adi\n- bahasa: Indonesia" },
      { id: "chatgpt", label: "ChatGPT", text: "- Nama: Abdi\n- bahasa: indonesia." },
    ]);
    expect(conflictsOf(m).map((k) => k.key)).toEqual(["nama"]);
    const bahasa = m.keyed.find((k) => k.key === "bahasa");
    expect(bahasa?.options).toEqual([{ value: "Indonesia", sources: ["gemini", "chatgpt"] }]);
    expect(unresolved(m, {})).toHaveLength(1);
    expect(buildMemory(m, {})).toBe("- bahasa: Indonesia\n");
    expect(buildMemory(m, { nama: 1 })).toBe("- nama: Abdi\n- bahasa: Indonesia\n");
  });

  it("kunci bernilai banyak digabung (bukan konflik); bisa dibuang sebagian dan ditambah", () => {
    const m = mergeMemory([
      { id: "a", label: "A", text: "- proyek: Zandor\n- alat: Git" },
      { id: "b", label: "B", text: "- proyek: Toko Online\n- kebiasaan unik: kopi pagi" },
    ]);
    expect(conflictsOf(m)).toEqual([]);
    expect(listsOf(m).map((k) => k.key)).toEqual(["proyek", "alat", "kebiasaan unik"]);
    expect(buildMemory(m, {})).toBe(
      "- proyek: Zandor\n- proyek: Toko Online\n- alat: Git\n- kebiasaan unik: kopi pagi\n",
    );
    const picks = { proyek: { excluded: [valueId("Toko Online")], extra: ["Blog  pribadi", "zandor", " "] } };
    expect(buildMemory(m, {}, picks)).toBe(
      "- proyek: Zandor\n- proyek: Blog pribadi\n- alat: Git\n- kebiasaan unik: kopi pagi\n",
    );
  });

  it("kunci tunggal dikenali dalam ID dan EN", () => {
    expect(isSingleKey("Nama")).toBe(true);
    expect(isSingleKey("Time Zone")).toBe(true);
    expect(isSingleKey("proyek")).toBe(false);
  });

  it("konflik bisa diisi nilai tulisan sendiri; kosong dianggap belum dipilih", () => {
    const m = mergeMemory([
      { id: "a", label: "A", text: "- nama: Adi" },
      { id: "b", label: "B", text: "- nama: Abdi" },
    ]);
    expect(unresolved(m, { nama: { custom: "  " } })).toHaveLength(1);
    expect(unresolved(m, { nama: { custom: "Abdi Guna" } })).toHaveLength(0);
    expect(buildMemory(m, { nama: { custom: " Abdi\n Guna " } })).toBe("- nama: Abdi Guna\n");
  });

  it("baris yang memuat secret dibuang", () => {
    const m = mergeMemory([
      { id: "x", label: "X", text: "- kunci: sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789\n- nama: Rina" },
    ]);
    expect(m.secrets).toBe(1);
    expect(buildMemory(m, {})).toBe("- nama: Rina\n");
  });

  it("kalimat bebas yang sama tidak diduplikasi", () => {
    const m = mergeMemory([
      { id: "a", label: "A", text: "- suka kopi" },
      { id: "b", label: "B", text: "- Suka kopi." },
    ]);
    expect(m.free).toEqual([{ value: "suka kopi", sources: ["a", "b"] }]);
  });
});
