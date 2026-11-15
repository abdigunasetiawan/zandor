// Mengubah berkas gambar pilihan pengguna menjadi data URL kecil untuk logo provider/model. Raster dikecilkan lewat
// canvas (maks LOGO_PX) supaya muat di penyimpanan; SVG dipakai apa adanya (ditampilkan lewat <img>, tidak menjalankan skrip).
import { isValidLogo, MAX_LOGO_CHARS } from "@zandor/core";

export const LOGO_PX = 128;
export const LOGO_ACCEPT = "image/png,image/jpeg,image/webp,image/gif,image/svg+xml";

export type LogoFileError = "type" | "size" | "decode";

function readAsDataUrl(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(new Error("decode"));
    r.readAsDataURL(file);
  });
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("decode"));
    img.src = src;
  });
}

/** Mengembalikan data URL logo yang valid, atau melempar Error dengan pesan `LogoFileError`. */
export async function fileToLogo(file: File): Promise<string> {
  if (!LOGO_ACCEPT.split(",").includes(file.type)) throw new Error("type");
  const raw = await readAsDataUrl(file);
  if (file.type === "image/svg+xml") {
    if (!isValidLogo(raw)) throw new Error(raw.length > MAX_LOGO_CHARS ? "size" : "decode");
    return raw;
  }
  const img = await loadImage(raw);
  const scale = Math.min(1, LOGO_PX / Math.max(img.naturalWidth, img.naturalHeight, 1));
  const w = Math.max(1, Math.round(img.naturalWidth * scale));
  const h = Math.max(1, Math.round(img.naturalHeight * scale));
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("decode");
  ctx.drawImage(img, 0, 0, w, h);
  const out = canvas.toDataURL("image/png");
  if (!isValidLogo(out)) throw new Error("size");
  return out;
}
