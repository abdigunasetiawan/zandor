// Plugin opencode Zandor. Fase 1: hanya guard env (wajib sejak engine pertama jalan, ADR-022).
// Memori & skill creator (ADR-023) ditambahkan di Fase 9.
import type { Plugin } from "@opencode-ai/plugin";
import { scrubEnv } from "./env-guard";

export const ZandorPlugin: Plugin = async () => ({
  "shell.env": async (_input, output) => {
    Object.assign(output.env, scrubEnv(process.env));
  },
});
