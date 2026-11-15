// Info model dari respons `GET {base}/models`. Standar OpenAI hanya memberi `id`; banyak server menambah metadata
// (OpenRouter, LM Studio, vLLM, LiteLLM, dll). Yang tidak disediakan endpoint dibiarkan kosong (tidak ditebak).
// Padanan Rust: `netprobe.rs::parse_model_info` (jaga keduanya sejalan).
import type { ProviderModelSpec } from "./store";

export type ModelMeta = Omit<ProviderModelSpec, "id">;

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const posInt = (v: unknown): number | undefined =>
  typeof v === "number" && Number.isFinite(v) && v > 0 ? Math.round(v) : undefined;
const strList = (v: unknown): string[] =>
  Array.isArray(v) ? v.filter((x): x is string => typeof x === "string").map((x) => x.toLowerCase()) : [];

function metaOf(m: Record<string, unknown>, id: string): ModelMeta {
  const top = isObj(m.top_provider) ? m.top_provider : {};
  const arch = isObj(m.architecture) ? m.architecture : {};
  const params = strList(m.supported_parameters);
  const caps = strList(m.capabilities);
  const inputs = strList(arch.input_modalities);
  const out: ModelMeta = {};
  const name = [m.name, m.display_name, m.displayName].find((n): n is string => typeof n === "string" && !!n.trim());
  if (name && name.trim() !== id) out.name = name.trim();
  const context =
    posInt(m.context_length) ??
    posInt(m.context_window) ??
    posInt(m.max_context_length) ??
    posInt(m.max_model_len) ??
    posInt(top.context_length);
  if (context) out.context = context;
  const output = posInt(top.max_completion_tokens) ?? posInt(m.max_output_tokens) ?? posInt(m.max_completion_tokens);
  if (output) out.output = output;
  if (params.length || caps.length) {
    const has = (...keys: string[]) => keys.some((k) => params.includes(k) || caps.includes(k));
    if (has("reasoning", "include_reasoning", "thinking")) out.reasoning = true;
    if (has("tools", "tool_use", "tool_choice", "function_calling")) out.toolCall = true;
  }
  if (inputs.includes("image") || caps.some((c) => c === "vision" || c === "image") || m.type === "vlm")
    out.attachment = true;
  return out;
}

/** id model → metadata yang tersedia. Hanya entri yang punya minimal satu info tambahan. */
export function parseModelInfo(body: unknown): Record<string, ModelMeta> {
  const list = isObj(body) && Array.isArray(body.data) ? body.data : Array.isArray(body) ? body : [];
  const out: Record<string, ModelMeta> = {};
  for (const m of list) {
    if (!isObj(m) || typeof m.id !== "string" || !m.id) continue;
    const meta = metaOf(m, m.id);
    if (Object.keys(meta).length > 0) out[m.id] = meta;
  }
  return out;
}

/** 128000 → "128K", 1000000 → "1M". */
export function compactTokens(n: number): string {
  if (n >= 1_000_000) return `${+(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1000) return `${Math.round(n / 1000)}K`;
  return String(n);
}
