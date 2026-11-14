// Perkecil gambar yang diunggah pengguna (latar/logo project) menjadi data URL kecil lewat canvas, supaya aman disimpan di
// penyimpanan lokal. Hanya untuk gambar raster; SVG ditolak (bisa membawa skrip).
export const IMAGE_TYPES = ["image/png", "image/jpeg", "image/webp", "image/gif"] as const;

export async function imageToDataUrl(
  file: File,
  opts: { maxSide: number; mime: "image/jpeg" | "image/png"; quality?: number; maxChars: number },
): Promise<string> {
  if (!(IMAGE_TYPES as readonly string[]).includes(file.type)) throw new Error("hanya png, jpeg, webp, atau gif");
  const bitmap = await createImageBitmap(file);
  try {
    const scale = Math.min(1, opts.maxSide / Math.max(bitmap.width, bitmap.height));
    const w = Math.max(1, Math.round(bitmap.width * scale));
    const h = Math.max(1, Math.round(bitmap.height * scale));
    const canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("canvas tidak tersedia");
    ctx.drawImage(bitmap, 0, 0, w, h);
    const url = canvas.toDataURL(opts.mime, opts.quality ?? 0.82);
    if (url.length > opts.maxChars) throw new Error("gambar terlalu besar setelah diperkecil; pilih gambar lain");
    return url;
  } finally {
    bitmap.close();
  }
}
