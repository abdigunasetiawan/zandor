// Input suara (dikte) dan daftar perangkat input untuk composer. Memakai Web Speech API milik WebView/browser:
// tersedia atau tidaknya (dan apakah memakai layanan jaringan) ditentukan lingkungan, jadi kegagalan dilaporkan apa adanya
// (bukan dipalsukan). Perangkat yang dipilih diteruskan lewat `SpeechRecognition.start(track)` (Chromium baru); bila engine
// tidak mendukungnya, pengenalan memakai perangkat bawaan sistem.
import { useCallback, useEffect, useRef, useState } from "react";

type RecognitionResult = { isFinal: boolean; 0?: { transcript: string } };
type RecognitionEvent = { resultIndex: number; results: ArrayLike<RecognitionResult> };
type Recognition = {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onresult: ((e: RecognitionEvent) => void) | null;
  onerror: ((e: { error: string }) => void) | null;
  onend: (() => void) | null;
  start(track?: MediaStreamTrack): void;
  stop(): void;
  abort(): void;
};
type RecognitionCtor = new () => Recognition;

export function speechRecognitionCtor(): RecognitionCtor | null {
  const w = globalThis as unknown as { SpeechRecognition?: RecognitionCtor; webkitSpeechRecognition?: RecognitionCtor };
  return w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null;
}

/** Kode bahasa pengenalan suara dari locale aplikasi. */
export function speechLang(locale: string): string {
  return locale === "id" ? "id-ID" : "en-US";
}

export type AudioInput = { deviceId: string; label: string };

const DEVICE_KEY = "zandor.voice.device";

export function loadVoiceDevice(): string {
  try {
    return localStorage.getItem(DEVICE_KEY) ?? "";
  } catch {
    return "";
  }
}

export function saveVoiceDevice(id: string): void {
  try {
    if (id) localStorage.setItem(DEVICE_KEY, id);
    else localStorage.removeItem(DEVICE_KEY);
  } catch {
    // penyimpanan tidak tersedia → pilihan hanya berlaku di sesi ini
  }
}

/** Perangkat input audio nyata dari sistem; diperbarui saat perangkat dicolok/dicabut. Label kosong sebelum izin mikrofon. */
export function useAudioInputs(): { devices: AudioInput[]; refresh: () => void } {
  const [devices, setDevices] = useState<AudioInput[]>([]);
  const refresh = useCallback(() => {
    const md = typeof navigator === "undefined" ? undefined : navigator.mediaDevices;
    if (!md?.enumerateDevices) return;
    md.enumerateDevices().then(
      (all) =>
        setDevices(
          all
            .filter((d) => d.kind === "audioinput" && d.deviceId)
            .map((d) => ({ deviceId: d.deviceId, label: d.label })),
        ),
      () => setDevices([]),
    );
  }, []);
  useEffect(() => {
    refresh();
    const md = typeof navigator === "undefined" ? undefined : navigator.mediaDevices;
    md?.addEventListener?.("devicechange", refresh);
    return () => md?.removeEventListener?.("devicechange", refresh);
  }, [refresh]);
  return { devices, refresh };
}

type DictationOptions = {
  lang: string;
  deviceId: string;
  /** Teks akhir (sudah final) dari pengenalan; dipanggil tiap ada ucapan selesai. */
  onText: (text: string) => void;
  /** Galat nyata dari engine (mis. not-allowed, network, service-not-allowed). */
  onError: (error: string) => void;
};

export function useDictation(opts: DictationOptions) {
  const supported = speechRecognitionCtor() !== null;
  const [listening, setListening] = useState(false);
  const recRef = useRef<Recognition | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const optsRef = useRef(opts);
  optsRef.current = opts;

  const release = useCallback(() => {
    for (const t of streamRef.current?.getTracks() ?? []) t.stop();
    streamRef.current = null;
    recRef.current = null;
    setListening(false);
  }, []);

  const stop = useCallback(() => recRef.current?.stop(), []);

  const start = useCallback(async () => {
    const Ctor = speechRecognitionCtor();
    if (!Ctor || recRef.current) return;
    const rec = new Ctor();
    rec.lang = optsRef.current.lang;
    rec.continuous = true;
    rec.interimResults = false;
    rec.onresult = (e) => {
      let text = "";
      for (let i = e.resultIndex; i < e.results.length; i++) {
        const r = e.results[i];
        if (r?.isFinal) text += r[0]?.transcript ?? "";
      }
      if (text.trim()) optsRef.current.onText(text.trim());
    };
    rec.onerror = (e) => {
      if (e.error !== "no-speech" && e.error !== "aborted") optsRef.current.onError(e.error);
    };
    rec.onend = release;
    recRef.current = rec;
    let track: MediaStreamTrack | undefined;
    const deviceId = optsRef.current.deviceId;
    if (deviceId) {
      try {
        streamRef.current = await navigator.mediaDevices.getUserMedia({ audio: { deviceId: { exact: deviceId } } });
        track = streamRef.current.getAudioTracks()[0];
      } catch (e) {
        optsRef.current.onError(e instanceof Error ? e.message : String(e));
        release();
        return;
      }
    }
    try {
      if (track) rec.start(track);
      else rec.start();
      setListening(true);
    } catch (e) {
      optsRef.current.onError(e instanceof Error ? e.message : String(e));
      release();
    }
  }, [release]);

  useEffect(() => () => recRef.current?.abort(), []);

  return { supported, listening, toggle: () => (recRef.current ? stop() : void start()) };
}
