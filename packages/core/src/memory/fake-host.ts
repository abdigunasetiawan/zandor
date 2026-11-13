// Host memori di memori proses: dipakai tes dan pratinjau browser. Meniru aturan host Rust (daftar putih berkas,
// nama skill, tolak menimpa tanpa `overwrite`) sehingga store diuji terhadap kontrak yang sama.
import type { MemoryFile, MemoryHost, MemoryScope, SkillInfo } from "./store";

const KNOWN =
  /^(settings\.json|global\.md|skill-queue\.jsonl|meta\.json|projects\/[0-9a-f]{16}\.md|pins\/[\w-]{1,80}\.md)$/;
const NAME = /^[a-z0-9]+(-[a-z0-9]+)*$/;

export type FakeMemoryHost = MemoryHost & {
  files: Map<string, string>;
  skills: Map<string, string>;
};

export function createFakeMemoryHost(options: { connected?: boolean } = {}): FakeMemoryHost {
  const files = new Map<string, string>();
  const skills = new Map<string, string>(); // `${scope}|${project ?? ""}|${name}` → markdown
  const known = (rel: string) => {
    if (!KNOWN.test(rel)) throw new Error(`berkas memori tidak dikenal: ${rel}`);
  };
  const skillKey = (scope: MemoryScope, project: string | null, name: string) => {
    if (!NAME.test(name) || name.length > 64) throw new Error("nama skill tidak valid");
    if (scope === "project" && !project) throw new Error("project wajib untuk skill project");
    return `${scope}|${scope === "project" ? project : ""}|${name}`;
  };
  return {
    connected: options.connected ?? true,
    files,
    skills,
    async list(): Promise<MemoryFile[]> {
      return [...files.entries()].map(([rel, text]) => ({ rel, size: text.length, modified: 0 }));
    },
    async read(rel) {
      known(rel);
      return files.get(rel) ?? "";
    },
    async write(rel, text) {
      known(rel);
      if (rel === "meta.json") throw new Error("meta.json hanya dibaca");
      files.set(rel, text);
    },
    async remove(rel) {
      known(rel);
      files.delete(rel);
    },
    async skillList(scope, project): Promise<SkillInfo[]> {
      const prefix = `${scope}|${scope === "project" ? project : ""}|`;
      return [...skills.keys()]
        .filter((k) => k.startsWith(prefix))
        .map((k) => ({ name: k.slice(prefix.length), scope, path: k }))
        .sort((a, b) => a.name.localeCompare(b.name));
    },
    async skillRead(scope, project, name) {
      const v = skills.get(skillKey(scope, project, name));
      if (v === undefined) throw new Error("skill tidak ditemukan");
      return v;
    },
    async skillWrite(scope, project, name, markdown, overwrite) {
      const k = skillKey(scope, project, name);
      if (skills.has(k) && !overwrite) throw new Error(`skill "${name}" sudah ada`);
      skills.set(k, markdown);
      return k;
    },
    async skillDelete(scope, project, name) {
      skills.delete(skillKey(scope, project, name));
    },
  };
}
