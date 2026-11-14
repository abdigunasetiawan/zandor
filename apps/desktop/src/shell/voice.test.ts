// @vitest-environment jsdom
import { afterEach, describe, expect, test, vi } from "vitest";
import { loadVoiceDevice, saveVoiceDevice, speechLang, speechRecognitionCtor } from "./voice";

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("voice", () => {
  test("bahasa pengenalan mengikuti locale aplikasi", () => {
    expect(speechLang("id")).toBe("id-ID");
    expect(speechLang("en")).toBe("en-US");
  });
  test("tanpa Web Speech API → tidak didukung (bukan dipalsukan)", () => {
    vi.stubGlobal("SpeechRecognition", undefined);
    vi.stubGlobal("webkitSpeechRecognition", undefined);
    expect(speechRecognitionCtor()).toBeNull();
  });
  test("ada Web Speech API → konstruktor dikembalikan", () => {
    class Fake {}
    vi.stubGlobal("webkitSpeechRecognition", Fake);
    expect(speechRecognitionCtor()).toBe(Fake);
  });
  test("perangkat terpilih tersimpan dan bisa dihapus (bawaan sistem)", () => {
    expect(loadVoiceDevice()).toBe("");
    saveVoiceDevice("abc");
    expect(loadVoiceDevice()).toBe("abc");
    saveVoiceDevice("");
    expect(loadVoiceDevice()).toBe("");
  });
});
