// Plugin opencode Zandor. Fase 4: guard env (wajib, ADR-022) + penanda "sudah termuat" yang diverifikasi supervisor.
// Memori & skill creator (ADR-023) ditambahkan di Fase 9.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import type { Plugin } from "@opencode-ai/plugin";
import { scrubEnv } from "./env-guard";

export const ZandorPlugin: Plugin = async (_input, options) => {
  // Supervisor menghapus penanda sebelum start lalu memeriksanya sesudah engine siap. Tanpa penanda, perlindungan
  // env tidak aktif dan engine TIDAK boleh dipakai.
  const marker = typeof options?.markerFile === "string" ? options.markerFile : null;
  if (marker) {
    mkdirSync(dirname(marker), { recursive: true });
    writeFileSync(marker, JSON.stringify({ guard: "shell.env", pid: process.pid, at: Date.now() }));
  }
  return {
    "shell.env": async (_input, output) => {
      Object.assign(output.env, scrubEnv(process.env));
    },
  };
};
