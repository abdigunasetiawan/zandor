// Adapter MemoryHost. Di Tauri: perintah Rust yang membaca/menulis `<app_data>/memory` dan folder skill opencode
// (berkas yang sama dibaca plugin Zandor di engine). Di luar Tauri (dev browser/e2e): pratinjau di memori proses,
// `connected=false` sehingga UI memberi tahu bahwa data TIDAK dibaca engine dan hilang saat dimuat ulang.
import { createFakeMemoryHost, type MemoryFile, type MemoryHost, type MemoryScope, type SkillInfo } from "@zandor/core";

export async function loadMemoryHost(): Promise<MemoryHost> {
  const { isTauri, invoke } = await import("@tauri-apps/api/core");
  if (!isTauri()) return createFakeMemoryHost({ connected: false });
  return {
    connected: true,
    list: () => invoke<MemoryFile[]>("memory_list"),
    read: (rel) => invoke<string>("memory_read", { rel }),
    async write(rel, text) {
      await invoke("memory_write", { rel, text });
    },
    async remove(rel) {
      await invoke("memory_delete", { rel });
    },
    skillList: (scope: MemoryScope, project) => invoke<SkillInfo[]>("skill_list", { scope, project }),
    skillRead: (scope, project, name) => invoke<string>("skill_read", { scope, project, name }),
    skillWrite: (scope, project, name, markdown, overwrite) =>
      invoke<string>("skill_write", { scope, project, name, markdown, overwrite }),
    async skillDelete(scope, project, name) {
      await invoke("skill_delete", { scope, project, name });
    },
  };
}
