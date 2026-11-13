// Sekali jalan (Fase 1): pecah baris ADR 001–018 di docs/decisions.md menjadi file
// docs/decisions/ADR-NNN-<slug>.md. Baris indeks di decisions.md tetap dipertahankan.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const index = readFileSync(join(root, "docs", "decisions.md"), "utf8");
const rows = index
  .split("\n")
  .map((l) => l.match(/^\|\s*(\d{3})\s*\|\s*([^|]+?)\s*\|\s*(.+?)\s*\|\s*(.+?)\s*\|\s*$/))
  .filter((m): m is RegExpMatchArray => !!m && Number(m[1]) <= 18);

const slug = (s: string) =>
  s
    .toLowerCase()
    .replace(/`/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .split("-")
    .slice(0, 6)
    .join("-");

for (const [, nnn, date, decision, reason] of rows) {
  const file = join(root, "docs", "decisions", `ADR-${nnn}-${slug(decision ?? "")}.md`);
  if (existsSync(file)) continue;
  const title = (decision ?? "").split(/[.(]/)[0]?.trim();
  writeFileSync(
    file,
    `# ADR-${nnn} — ${title}

- **Tanggal:** ${date}
- **Status:** Diterima

## Keputusan

${decision}

## Alasan

${reason}

> Dipecah dari tabel \`docs/decisions.md\` saat Fase 1 tanpa perubahan isi. Perubahan berikutnya dicatat
> sebagai ADR baru yang merujuk ADR ini.
`,
  );
  console.log("dibuat", file.slice(root.length + 1));
}
