// Menghasilkan packages/ui/src/model-logos.generated.ts dari paket MIT `@lobehub/icons-static-svg` (ikon monokrom).
//
// Pemakaian: unduh paket ke folder sementara yang kosong, lalu
//   bun scripts/gen-model-logos.ts <folder-paket>/package/icons
// Hanya ikon bertipe <path> tunggal/jamak yang diambil (tanpa <g>, <circle>, gradient, dsb) agar dapat dirender sebagai
// elemen React biasa dengan warna `currentColor` (tanpa dangerouslySetInnerHTML). Logo adalah merek dagang pemiliknya;
// dipakai hanya untuk mengenali model/provider (lihat docs/THIRD-PARTY-LICENSES.md).
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const dir = process.argv[2];
if (!dir) throw new Error("Pemakaian: bun scripts/gen-model-logos.ts <folder icons>");

// Ikon model/provider/agent yang relevan untuk aplikasi agentic harness.
const KEYS =
  `anthropic claude claudecode openai codex gemini geminicli gemma google googlecloud vertexai aistudio deepmind
antigravity deepseek qwen alibaba alibabacloud bailian meta metaai microsoft azure azureai copilot githubcopilot github
mistral cohere perplexity xai grok groq cerebras sambanova together fireworks deepinfra hyperbolic nebius novita
openrouter ollama lmstudio huggingface vllm nvidia ibm aws bedrock cloudflare workersai vercel replicate modelscope
moonshot kimi minimax zhipu zai chatglm glmv baichuan stepfun yi doubao volcengine bytedance hunyuan tencent baidu wenxin
spark internlm sensenova siliconcloud ai21 ai360 inflection nousresearch nova liquid upstage reka arcee poolside sakana
stability flux midjourney runway luma suno elevenlabs cursor windsurf opencode cline roocode kilocode openhands goose
devin trae kiro junie lovable replit v0 zencoder codegeex mcp langchain llamaindex crewai dify coze n8n openwebui
sillytavern poe straico jina voyage lepton leptonai friendli featherless infermatic chutes parasail ppio targon venice
unsloth rwkv tii llava dbrx aya cogview cogvideo kling hailuo vidu pika pixverse sora dalle ideogram krea recraft`
    .split(/\s+/)
    .filter(Boolean);

type Logo = { v: string; p: Array<{ d: string; e?: boolean }> };
const out: Record<string, Logo> = {};
const skipped: string[] = [];
const color: Record<string, string> = {};

for (const key of [...new Set(KEYS)]) {
  let svg: string;
  try {
    svg = readFileSync(join(dir, `${key}.svg`), "utf8");
  } catch {
    skipped.push(`${key} (tidak ada)`);
    continue;
  }
  // Varian berwarna (warna merek, bisa bergradasi) bila ada: disimpan sebagai string SVG utuh, dirender lewat <img data:>.
  try {
    const raw = readFileSync(join(dir, `${key}-color.svg`), "utf8");
    color[key] = raw
      .replace(/<title>[^<]*<\/title>/, "")
      .replace(/\sstyle="[^"]*"/, "")
      .replace(/\s(width|height)="1em"/g, "")
      .trim();
  } catch {
    // tidak ada varian berwarna → pakai monokrom (currentColor)
  }
  const viewBox = /viewBox="([^"]+)"/.exec(svg)?.[1];
  const body = svg.replace(/<title>[^<]*<\/title>/, "").replace(/^[\s\S]*?<svg[^>]*>/, "");
  const inner = body.replace(/<\/svg>\s*$/, "");
  const tags = [...inner.matchAll(/<(\w+)([^>]*)>/g)];
  if (!viewBox || tags.length === 0 || tags.some((t) => t[1] !== "path" || /fill="(?!currentColor)/.test(t[2] ?? ""))) {
    skipped.push(`${key} (bukan path monokrom)`);
    continue;
  }
  const rootEvenOdd = /fill-rule="evenodd"/.test(svg.split(">")[0] ?? "");
  out[key] = {
    v: viewBox,
    p: tags.map((t) => ({
      d: /\sd="([^"]+)"/.exec(t[2] ?? "")?.[1] ?? "",
      ...(rootEvenOdd || /fill-rule="evenodd"/.test(t[2] ?? "") ? { e: true } : {}),
    })),
  };
}

const text = `// DIHASILKAN oleh scripts/gen-model-logos.ts dari @lobehub/icons-static-svg (MIT). Jangan edit manual.
// Logo adalah merek dagang pemiliknya; dipakai hanya untuk mengenali model/provider.
export type ModelLogoData = { v: string; p: ReadonlyArray<{ d: string; e?: boolean }> };

export const MODEL_LOGOS: Readonly<Record<string, ModelLogoData>> = ${JSON.stringify(out, null, 1)};

// Varian berwarna merek (SVG utuh). Dirender sebagai gambar data: (tanpa skrip), bukan lewat DOM.
export const MODEL_LOGOS_COLOR: Readonly<Record<string, string>> = ${JSON.stringify(color, null, 1)};
`;
writeFileSync(join(import.meta.dir, "..", "packages", "ui", "src", "model-logos.generated.ts"), text);
console.log(
  `${Object.keys(out).length} logo ditulis (${Object.keys(color).length} berwarna). Dilewati: ${skipped.join(", ") || "-"}`,
);
