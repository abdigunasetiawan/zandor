// Bahasa Indonesia = sumber kebenaran kunci terjemahan. Bahasa lain wajib punya kunci yang sama
// (dicek compiler lewat tipe `Messages` dan oleh test placeholder).
export const id = {
  "app.name": "Zandor",
  "app.version": "Versi {version}",
  "shell.emptyTitle": "Kerangka aplikasi",
  "shell.emptyBody": "Fitur belum tersedia. Panel akan muncul seiring fase pengembangan.",
  "settings.language": "Bahasa",
  "language.id": "Indonesia",
  "language.en": "English",
  "error.panelTitle": "Bagian ini mengalami galat",
  "error.panelBody": "Bagian lain aplikasi tetap berjalan. Detail galat tercatat di log.",
  "error.retry": "Coba lagi",
  "ui.loading": "Memuat",
  "ui.close": "Tutup",
  "ui.clear": "Bersihkan",
  "ui.search": "Cari",
  "ui.noResults": "Tidak ada hasil",
  "ui.confirm": "Ya, lanjutkan",
  "ui.cancel": "Batal",
  "ui.notifications": "Notifikasi",
  "theme.label": "Tema",
  "theme.dark": "Gelap",
  "theme.light": "Terang",
  "theme.system": "Ikuti sistem",
  "scale.label": "Skala UI",
} as const;

export type MessageKey = keyof typeof id;
export type Messages = Record<MessageKey, string>;
