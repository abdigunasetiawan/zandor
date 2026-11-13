// Hasilkan docs/changelog.md dari riwayat commit (judul commit, dikelompokkan per tanggal).
import { execSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";

const log = execSync('git log --date=short --pretty=format:"%ad%x09%h%x09%s"', { encoding: "utf8" });
const byDate = new Map<string, string[]>();
for (const line of log.split("\n").filter(Boolean)) {
  const [date, hash, subject] = line.split("\t");
  if (!date || !hash || !subject) continue;
  byDate.set(date, [...(byDate.get(date) ?? []), `- ${subject} (\`${hash}\`)`]);
}
const body = [...byDate].map(([d, items]) => `## ${d}\n\n${items.join("\n")}`).join("\n\n");
writeFileSync(
  join(import.meta.dir, "..", "docs", "changelog.md"),
  `# Changelog\n\n> Dihasilkan oleh \`bun scripts/changelog.ts\`. Jangan diedit manual.\n\n${body}\n`,
);
