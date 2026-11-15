// Logo model/provider (monokrom, currentColor). Dipilih dari keluarga model dahulu (mis. gemini-*), lalu provider; bila
// tidak ada yang cocok jatuh ke tile inisial (Avatar) agar tampilan tidak pernah kosong. Data: model-logos.generated.ts.
import { createContext, type ReactNode, useContext } from "react";
import { cx } from "../cx";
import { MODEL_LOGOS, MODEL_LOGOS_COLOR } from "../model-logos.generated";
import { Avatar } from "./Avatar";

// [pola pada id model (huruf kecil), kunci logo]; urutan penting (yang spesifik lebih dulu).
const MODEL_PATTERNS: ReadonlyArray<readonly [RegExp, string]> = [
  [/gpt-oss/, "openai"],
  [/codex/, "codex"],
  [/claude|\bhaiku\b|\bopus\b|\bsonnet\b/, "claude"],
  [/gpt|chatgpt|(^|[/:-])o[1-9](-|$)|davinci|whisper|dall-?e/, "openai"],
  [/gemma/, "gemma"],
  [/gemini|bard|palm/, "gemini"],
  [/deepseek/, "deepseek"],
  [/qwen|qwq|qvq/, "qwen"],
  [/(?<!o)llama|llava/, "meta"],
  [/mistral|mixtral|codestral|ministral|magistral|devstral|pixtral/, "mistral"],
  [/grok/, "grok"],
  [/glm|chatglm/, "zhipu"],
  [/kimi/, "kimi"],
  [/moonshot/, "moonshot"],
  [/minimax|abab/, "minimax"],
  [/command|cohere|\baya\b/, "cohere"],
  [/sonar|perplexity/, "perplexity"],
  [/phi-?\d|\bphi\b/, "microsoft"],
  [/ernie|wenxin/, "wenxin"],
  [/doubao|seed-/, "doubao"],
  [/hunyuan/, "hunyuan"],
  [/step-/, "stepfun"],
  [/nemotron/, "nvidia"],
  [/granite/, "ibm"],
  [/internlm/, "internlm"],
  [/baichuan/, "baichuan"],
  [/spark/, "spark"],
  [/dbrx/, "dbrx"],
  [/hermes/, "nousresearch"],
  [/jamba/, "ai21"],
  [/nova-/, "nova"],
];

// Alias id provider (setelah dinormalisasi: huruf kecil tanpa tanda baca) → kunci logo.
const PROVIDER_ALIASES: Readonly<Record<string, string>> = {
  googlevertex: "vertexai",
  googleaistudio: "aistudio",
  amazonbedrock: "bedrock",
  zhipuai: "zhipu",
  zhipuaicodingplan: "zhipu",
  zai: "zai",
  zaicodingplan: "zai",
  moonshotai: "moonshot",
  moonshotaicn: "moonshot",
  kimiforcoding: "kimi",
  xai: "xai",
  togetherai: "together",
  fireworksai: "fireworks",
  deepinfra: "deepinfra",
  githubmodels: "github",
  githubcopilot: "githubcopilot",
  cloudflareworkersai: "workersai",
  cloudflareaigateway: "cloudflare",
  opencodezen: "opencode",
  azurecognitiveservices: "azure",
  huggingface: "huggingface",
  lmstudio: "lmstudio",
  llamacpp: "meta",
  alibabacn: "alibabacloud",
  alibaba: "alibabacloud",
  dashscope: "bailian",
  siliconflow: "siliconcloud",
  volcengine: "volcengine",
  minimaxcn: "minimax",
  minimaxcodingplan: "minimax",
  nvidia: "nvidia",
  stepfun: "stepfun",
};

const norm = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");

/** Deteksi dari teks bebas (nama tampilan): pola keluarga model, lalu kunci logo yang termuat di nama. */
export function detectLogoKey(text?: string | null): string | null {
  const t = (text ?? "").toLowerCase();
  if (!t) return null;
  const n = norm(t);
  // nama produk/provider yang jelas (≥5 huruf, mis. "ollama", "lmstudio") didahulukan; keluarga model sesudahnya
  const hit = Object.keys(MODEL_LOGOS)
    .filter((k) => k.length >= 5 && n.includes(k))
    .sort((a, b) => b.length - a.length)[0];
  if (hit) return hit;
  for (const [re, key] of MODEL_PATTERNS) if (re.test(t) && MODEL_LOGOS[key]) return key;
  return null;
}

/** Kunci logo untuk model/provider, atau null bila tidak ada yang cocok. `name` (nama tampilan) jadi cadangan terakhir. */
export function modelLogoKey(providerID?: string | null, modelID?: string | null, name?: string | null): string | null {
  const model = (modelID ?? "").toLowerCase();
  if (model) {
    for (const [re, key] of MODEL_PATTERNS) if (re.test(model) && MODEL_LOGOS[key]) return key;
  }
  const p = norm(providerID ?? "");
  if (p) {
    const alias = PROVIDER_ALIASES[p];
    if (alias && MODEL_LOGOS[alias]) return alias;
    if (MODEL_LOGOS[p]) return p;
    const trimmed = p.replace(/(ai|cn|api)$/, "");
    if (trimmed && MODEL_LOGOS[trimmed]) return trimmed;
  }
  return detectLogoKey(name);
}

/** Logo kustom pengguna (data URL gambar): model lebih dulu, lalu provider. */
export type LogoOverrides = (providerID?: string | null, modelID?: string | null) => string | null;
const OverrideContext = createContext<LogoOverrides | null>(null);
export function ModelLogoOverrides({ resolve, children }: { resolve: LogoOverrides; children: ReactNode }) {
  return <OverrideContext.Provider value={resolve}>{children}</OverrideContext.Provider>;
}

// Varian berwarna dirender sebagai gambar data: (SVG di dalam <img> tidak menjalankan skrip dan tidak memengaruhi DOM).
const uriCache = new Map<string, string>();
function colorUri(key: string): string | null {
  const svg = MODEL_LOGOS_COLOR[key];
  if (!svg) return null;
  let uri = uriCache.get(key);
  if (!uri) {
    uri = `data:image/svg+xml,${encodeURIComponent(svg)}`;
    uriCache.set(key, uri);
  }
  return uri;
}

export type ModelLogoProps = {
  providerID?: string | null;
  modelID?: string | null;
  /** Teks pengganti untuk tile inisial saat logo tidak ada (default: id provider atau model). */
  name?: string;
  className?: string;
  /** Abaikan logo kustom pengguna dan tampilkan hasil deteksi otomatis (untuk pratinjau di editor logo). */
  auto?: boolean;
};

export function ModelLogo({ providerID, modelID, name, className, auto }: ModelLogoProps) {
  const overrides = useContext(OverrideContext);
  const custom = auto ? null : (overrides?.(providerID, modelID) ?? null);
  if (custom)
    return (
      <img
        className={cx("z-model-logo", className)}
        src={custom}
        alt=""
        aria-hidden="true"
        draggable={false}
        data-logo="custom"
        data-color=""
      />
    );
  const key = modelLogoKey(providerID, modelID, name);
  const color = key ? colorUri(key) : null;
  if (key && color)
    return (
      <img
        className={cx("z-model-logo", className)}
        src={color}
        alt=""
        aria-hidden="true"
        draggable={false}
        data-logo={key}
        data-color=""
      />
    );
  const data = key ? MODEL_LOGOS[key] : undefined;
  if (!data)
    return <Avatar name={name || providerID || modelID || "?"} size="sm" className={cx("z-model-logo", className)} />;
  return (
    <svg
      className={cx("z-model-logo", className)}
      viewBox={data.v}
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
      data-logo={key}
    >
      {data.p.map((p) => (
        <path key={p.d} d={p.d} fillRule={p.e ? "evenodd" : undefined} />
      ))}
    </svg>
  );
}
