// Spike S2 — render transcript (HTML string) dari state reducer.
// `switch` atas `Part["type"]` bersifat exhaustive: bila SDK menambah jenis part baru,
// `tsc` gagal di `assertNever` → kelengkapan pemetaan dijaga oleh compiler.
import type { AssistantMessage, Message, Part, ToolPart } from "@opencode-ai/sdk/v2/types";
import { orderedMessages, permissionsForCall, type PermissionView, type SessionView } from "./reducer";

const esc = (s: unknown) =>
  String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

function assertNever(x: never): never {
  throw new Error("Part tidak dikenal: " + JSON.stringify(x));
}

const fmtMs = (ms: number) => (ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} dtk`);

function renderPermission(p: PermissionView): string {
  const label = p.reply === "once" ? "diizinkan sekali" : p.reply === "always" ? "diizinkan selalu" : p.reply === "reject" ? "ditolak" : "menunggu persetujuan";
  return `<div class="perm perm-${esc(p.reply ?? "pending")}" data-permission="${esc(p.id)}">🔐 Izin <b>${esc(p.permission)}</b> · <code>${esc(p.patterns.join(", "))}</code> — ${label}</div>`;
}

function renderTool(part: ToolPart, s: SessionView): string {
  const st = part.state;
  const input = st.input as Record<string, unknown>;
  const perms = permissionsForCall(s, part.callID).map(renderPermission).join("");
  const dur = st.status === "completed" || st.status === "error" ? ` · ${fmtMs(st.time.end - st.time.start)}` : "";
  const head = (title: string) =>
    `<div class="tool-head"><span class="tool-name">${esc(part.tool)}</span> ${title} <span class="status status-${st.status}">${st.status}</span>${dur}</div>`;
  let body = "";
  const meta = (st.status === "pending" ? undefined : st.metadata) as Record<string, unknown> | undefined;
  switch (part.tool) {
    case "read":
      body = head(`<code>${esc(input.filePath)}</code>`) + (st.status === "completed" ? `<div class="muted">${esc(meta?.preview ? "isi file dibaca" : st.title)}</div>` : "");
      break;
    case "edit":
    case "write":
    case "apply_patch":
      body = head(`<code>${esc(input.filePath ?? "")}</code>`) + (meta?.diff ? `<pre class="diff">${esc(meta.diff)}</pre>` : "");
      break;
    case "bash":
      body =
        head(`<code>$ ${esc(input.command)}</code>`) +
        (meta?.output || st.status === "completed" ? `<pre class="term">${esc((meta?.output as string) ?? (st.status === "completed" ? st.output : ""))}</pre>` : "");
      break;
    case "todowrite": {
      const todos = (input.todos as { content: string; status: string }[] | undefined) ?? [];
      body = head("") + `<ul class="todos">${todos.map((t) => `<li class="todo-${esc(t.status)}">${esc(t.content)} <span class="muted">(${esc(t.status)})</span></li>`).join("")}</ul>`;
      break;
    }
    case "webfetch":
      body = head(`<code>${esc(input.url)}</code>`);
      break;
    case "websearch":
      body = head(`<code>${esc(input.query)}</code>`);
      break;
    case "task":
      body = head(esc(input.description));
      break;
    default:
      body = head(esc(st.status !== "pending" && "title" in st ? (st.title ?? "") : "")) + `<pre class="json">${esc(JSON.stringify(input, null, 2))}</pre>`;
  }
  if (st.status === "error") body += `<div class="error">${esc(st.error)}</div>`;
  return `<div class="part tool" data-call="${esc(part.callID)}">${body}${perms}</div>`;
}

export function renderPart(part: Part, s: SessionView): string {
  switch (part.type) {
    case "text":
      if (part.ignored) return "";
      return `<div class="part text${part.synthetic ? " synthetic" : ""}">${part.synthetic ? '<div class="muted">konteks sintetis</div>' : ""}<div class="md">${esc(part.text)}</div></div>`;
    case "reasoning":
      return `<details class="part reasoning"${part.time.end ? "" : " open"}><summary>💭 Thinking${part.time.end ? ` · ${fmtMs(part.time.end - part.time.start)}` : " …"}</summary><div class="md">${esc(part.text)}</div></details>`;
    case "file":
      return `<div class="part file">📎 <code>${esc(part.filename ?? part.url)}</code> <span class="muted">${esc(part.mime)}</span></div>`;
    case "tool":
      return renderTool(part, s);
    case "step-start":
      return "";
    case "step-finish":
      return `<div class="part step muted">langkah selesai (${esc(part.reason)}) · token in ${part.tokens.input} / out ${part.tokens.output} / reasoning ${part.tokens.reasoning}</div>`;
    case "snapshot":
      return "";
    case "patch":
      return `<div class="part patch">🩹 Perubahan file: ${part.files.map((f) => `<code>${esc(f)}</code>`).join(", ")} <span class="muted">(snapshot ${esc(part.hash.slice(0, 8))})</span></div>`;
    case "subtask":
      return `<div class="part subtask">↳ Sub-agent <b>${esc(part.agent)}</b>: ${esc(part.description)}</div>`;
    case "agent":
      return `<div class="part agent">@${esc(part.name)}</div>`;
    case "retry":
      return `<div class="part retry error">Percobaan ulang #${part.attempt}: ${esc(part.error.data?.message ?? part.error.name)}</div>`;
    case "compaction":
      return `<div class="part compaction muted">— konteks dipadatkan${part.auto ? " otomatis" : ""} —</div>`;
    default:
      return assertNever(part);
  }
}

function renderMeta(info: Message): string {
  if (info.role !== "assistant") return "";
  const a = info as AssistantMessage;
  const dur = a.time.completed ? ` · ${fmtMs(a.time.completed - a.time.created)}` : " · berjalan…";
  const when = new Date(a.time.created).toISOString();
  return `<div class="meta">${esc(a.modelID)} · ${esc(a.providerID)}${dur} · <time datetime="${when}" title="${when}">${when.slice(11, 19)}</time>${a.error ? ` · <span class="error">${esc(a.error.name)}</span>` : ""}</div>`;
}

export function renderTranscript(s: SessionView): string {
  const msgs = orderedMessages(s)
    .map(({ info, parts }) => {
      const inner = parts.map((p) => renderPart(p, s)).join("");
      return `<article class="msg ${esc(info.role)}" data-id="${esc(info.id)}">${inner}${renderMeta(info)}</article>`;
    })
    .join("\n");
  const todos = s.todos.length
    ? `<aside class="todos-panel"><b>Todo</b><ul>${s.todos.map((t) => `<li class="todo-${esc(t.status)}">${esc(t.content)}</li>`).join("")}</ul></aside>`
    : "";
  const pending = [...s.permissions.values()].filter((p) => !p.reply && !p.tool).map(renderPermission).join("");
  return `<section class="transcript" data-session="${esc(s.id)}" data-status="${esc(s.status.type)}">${msgs}${pending}${todos}</section>`;
}

export function renderDocument(s: SessionView, title: string): string {
  return `<!doctype html><html lang="id"><head><meta charset="utf-8"><title>${esc(title)}</title><style>
body{background:#121212;color:#e6e6e6;font:14px/1.5 system-ui,sans-serif;max-width:860px;margin:24px auto;padding:0 16px}
.msg{border:1px solid #262626;border-radius:8px;padding:12px;margin:12px 0;background:#141414}.msg.user{background:#1f1f1f}
.part{margin:6px 0}.md{white-space:pre-wrap}.muted{color:#8a8a8a;font-size:12px}.meta{color:#8a8a8a;font-size:12px;margin-top:8px}
pre{background:#0d0d0d;padding:8px;border-radius:6px;overflow:auto;font-size:12px}.tool{border-left:2px solid #444;padding-left:8px}
.tool-name{font-weight:600}.status{font-size:11px;padding:1px 6px;border-radius:4px;background:#262626}.status-error,.error{color:#ff8a8a}
.perm{font-size:12px;margin-top:4px}.perm-reject{color:#ff8a8a}.synthetic{opacity:.7}details summary{cursor:pointer;color:#aaa}
</style></head><body><h1>${esc(title)}</h1>${renderTranscript(s)}</body></html>`;
}
