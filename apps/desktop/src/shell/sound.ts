// Suara pemberitahuan: nada bawaan DISINTESIS dengan Web Audio (tanpa berkas aset), atau berkas suara milik pengguna per
// peristiwa (disimpan sebagai data URL di penyimpanan lokal, maks 1 MB). Pemutaran hanya terjadi atas peristiwa sesi atau
// tombol "Uji"; gagal memutar tidak pernah mengganggu aplikasi.
import type { SoundSetting } from "@zandor/core";

export type SoundEvent = "done" | "failed" | "action";
export const SOUND_EVENTS: readonly SoundEvent[] = ["done", "failed", "action"];
export const SOUND_CHOICES: readonly SoundSetting[] = [
  "chime",
  "pop",
  "bell",
  "soft",
  "alert",
  "low",
  "none",
  "custom",
];
export const MAX_CUSTOM_BYTES = 1024 * 1024;

const KEY = (ev: SoundEvent) => `zandor.sound.custom.${ev}`;
export type CustomSound = { name: string; url: string };

export function loadCustomSound(ev: SoundEvent): CustomSound | null {
  try {
    const raw = localStorage.getItem(KEY(ev));
    if (!raw) return null;
    const v = JSON.parse(raw) as Partial<CustomSound>;
    return typeof v.name === "string" && typeof v.url === "string" && v.url.startsWith("data:audio/")
      ? { name: v.name, url: v.url }
      : null;
  } catch {
    return null;
  }
}

export function clearCustomSound(ev: SoundEvent): void {
  try {
    localStorage.removeItem(KEY(ev));
  } catch {
    // penyimpanan tidak tersedia
  }
}

/** Simpan berkas suara pilihan pengguna. Menolak yang bukan audio atau lebih dari 1 MB (pesan galat berbahasa Indonesia). */
export async function saveCustomSound(ev: SoundEvent, file: File): Promise<CustomSound> {
  if (!file.type.startsWith("audio/")) throw new Error("berkas bukan audio");
  if (file.size > MAX_CUSTOM_BYTES) throw new Error(`berkas terlalu besar (maks ${MAX_CUSTOM_BYTES / 1024 / 1024} MB)`);
  const url = await new Promise<string>((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(new Error("gagal membaca berkas"));
    r.readAsDataURL(file);
  });
  const sound: CustomSound = { name: file.name, url };
  try {
    localStorage.setItem(KEY(ev), JSON.stringify(sound));
  } catch {
    throw new Error("penyimpanan lokal penuh atau tidak tersedia");
  }
  return sound;
}

let ctx: AudioContext | null = null;
function audio(): AudioContext | null {
  try {
    const Ctor =
      window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctor) return null;
    ctx ??= new Ctor();
    if (ctx.state === "suspended") void ctx.resume();
    return ctx;
  } catch {
    return null;
  }
}

type Note = { f: number; at: number; dur: number; type?: OscillatorType; gain?: number; to?: number };
/** Pola nada tiap suara bawaan (frekuensi Hz, mulai dan durasi dalam detik). */
export const PATTERNS: Record<Exclude<SoundSetting, "none" | "custom">, Note[]> = {
  chime: [
    { f: 1046.5, at: 0, dur: 0.22 },
    { f: 1318.5, at: 0.11, dur: 0.22 },
    { f: 1568, at: 0.22, dur: 0.4 },
  ],
  pop: [{ f: 620, at: 0, dur: 0.14, to: 180, gain: 0.9 }],
  bell: [
    { f: 880, at: 0, dur: 1.0 },
    { f: 1760, at: 0, dur: 0.6, gain: 0.35 },
    { f: 2640, at: 0, dur: 0.3, gain: 0.15 },
  ],
  soft: [
    { f: 523.25, at: 0, dur: 0.28, type: "triangle" },
    { f: 659.25, at: 0.14, dur: 0.34, type: "triangle" },
  ],
  alert: [
    { f: 880, at: 0, dur: 0.11, type: "square", gain: 0.35 },
    { f: 880, at: 0.18, dur: 0.11, type: "square", gain: 0.35 },
  ],
  low: [
    { f: 233, at: 0, dur: 0.3, gain: 0.9 },
    { f: 174.6, at: 0.2, dur: 0.45, gain: 0.9 },
  ],
};

/** Putar suara untuk satu peristiwa. Mengembalikan false bila tidak ada yang diputar (tanpa suara / tidak didukung / gagal). */
export async function playSound(choice: SoundSetting, volume: number, ev: SoundEvent): Promise<boolean> {
  const vol = Math.max(0, Math.min(1, volume));
  if (choice === "none" || vol === 0) return false;
  try {
    if (choice === "custom") {
      const c = loadCustomSound(ev);
      if (!c) return false;
      const a = new Audio(c.url);
      a.volume = vol;
      await a.play();
      return true;
    }
    const ac = audio();
    if (!ac) return false;
    const now = ac.currentTime;
    for (const n of PATTERNS[choice]) {
      const osc = ac.createOscillator();
      const g = ac.createGain();
      osc.type = n.type ?? "sine";
      osc.frequency.setValueAtTime(n.f, now + n.at);
      if (n.to) osc.frequency.exponentialRampToValueAtTime(n.to, now + n.at + n.dur);
      const peak = vol * 0.4 * (n.gain ?? 1);
      g.gain.setValueAtTime(0.0001, now + n.at);
      g.gain.exponentialRampToValueAtTime(Math.max(peak, 0.0002), now + n.at + 0.015);
      g.gain.exponentialRampToValueAtTime(0.0001, now + n.at + n.dur);
      osc.connect(g).connect(ac.destination);
      osc.start(now + n.at);
      osc.stop(now + n.at + n.dur + 0.05);
    }
    return true;
  } catch {
    return false;
  }
}
