// Gambar di percakapan hanya dimuat dari data: URI bertipe gambar (bukan dari jaringan), agar isi dari model tidak bisa
// mengeksfiltrasi data lewat URL gambar.
export const SAFE_IMAGE_URI = /^data:image\/(png|jpe?g|gif|webp|svg\+xml)[;,]/i;
