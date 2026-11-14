// Aset visual untuk sesi demo (gambar SVG buatan sendiri). Dikecualikan dari penjaga token (warna di dalam gambar, bukan gaya UI).

const svgUri = (svg: string) => `data:image/svg+xml;base64,${btoa(svg)}`;

/** Grafik batang sederhana (gambar, bukan Mermaid) — contoh gambar di respons. */
export const DEMO_CHART_IMAGE = svgUri(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 480 240" width="480" height="240">
  <rect width="480" height="240" rx="12" fill="#1f2430"/>
  <text x="24" y="34" font-family="sans-serif" font-size="16" fill="#e6e9ef">Waktu build (detik) per percobaan</text>
  <g font-family="sans-serif" font-size="12" fill="#9aa3b5">
    <text x="48" y="222">#1</text><text x="136" y="222">#2</text><text x="224" y="222">#3</text><text x="312" y="222">#4</text><text x="400" y="222">#5</text>
  </g>
  <g fill="#6ea8fe">
    <rect x="40" y="70" width="40" height="136" rx="4"/>
    <rect x="128" y="96" width="40" height="110" rx="4"/>
    <rect x="216" y="120" width="40" height="86" rx="4"/>
    <rect x="304" y="140" width="40" height="66" rx="4"/>
    <rect x="392" y="156" width="40" height="50" rx="4" fill="#7ee0a1"/>
  </g>
</svg>`,
);

/** Ilustrasi arsitektur ringkas (gambar kedua, sebagai part berkas). */
export const DEMO_DIAGRAM_IMAGE = svgUri(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 480 160" width="480" height="160">
  <rect width="480" height="160" rx="12" fill="#232a36"/>
  <g font-family="sans-serif" font-size="13" fill="#e6e9ef" text-anchor="middle">
    <rect x="24" y="56" width="120" height="48" rx="8" fill="#2f3b52"/><text x="84" y="85">Aplikasi (UI)</text>
    <rect x="180" y="56" width="120" height="48" rx="8" fill="#3b4a66"/><text x="240" y="85">Engine opencode</text>
    <rect x="336" y="56" width="120" height="48" rx="8" fill="#2f3b52"/><text x="396" y="85">Provider model</text>
  </g>
  <g stroke="#9aa3b5" stroke-width="2" fill="none">
    <path d="M144 80h36"/><path d="M300 80h36"/>
  </g>
</svg>`,
);
