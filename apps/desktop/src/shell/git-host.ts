// Adapter Git: perintah Rust (menjalankan `git` CLI di project terdaftar). Di luar Tauri tidak ada git → `available=false`
// dan panel menjelaskannya (tidak ada data palsu).
import type { Branch, CommitInfo, GitHost, GitStatus } from "@zandor/core";

const unavailable = (): never => {
  throw new Error("Git hanya tersedia di aplikasi desktop");
};

export function createUnavailableGitHost(): GitHost {
  return {
    available: false,
    status: async () => unavailable(),
    diff: async () => unavailable(),
    stage: async () => unavailable(),
    unstage: async () => unavailable(),
    commit: async () => unavailable(),
    branches: async () => unavailable(),
    switchBranch: async () => unavailable(),
    createBranch: async () => unavailable(),
    log: async () => unavailable(),
    repos: async () => [],
  };
}

export async function loadGitHost(): Promise<GitHost> {
  const { isTauri, invoke } = await import("@tauri-apps/api/core");
  if (!isTauri()) return createUnavailableGitHost();
  return {
    available: true,
    status: (root) => invoke<GitStatus>("git_status", { root }),
    diff: (root, path, staged, untracked) => invoke<string>("git_diff", { root, path, staged, untracked }),
    async stage(root, paths) {
      await invoke("git_stage", { root, paths });
    },
    async unstage(root, paths) {
      await invoke("git_unstage", { root, paths });
    },
    commit: (root, message) => invoke<string>("git_commit", { root, message }),
    branches: (root) => invoke<Branch[]>("git_branches", { root }),
    async switchBranch(root, name) {
      await invoke("git_switch", { root, name });
    },
    async createBranch(root, name) {
      await invoke("git_create_branch", { root, name });
    },
    log: (root, count) => invoke<CommitInfo[]>("git_log", { root, count }),
    repos: (root) => invoke<string[]>("git_repos", { root }),
  };
}
