// Panel 2: percakapan. Tab sesi, transcript dari ChatStore (SSE engine), aksi pesan (salin/cabang/revert/ulang),
// composer (lampiran, mode izin, reasoning, model), kartu izin, stop.

import { useVirtualizer } from "@tanstack/react-virtual";
import {
  type ChatMessage,
  type FileAttachment,
  type MessagePart,
  messageText,
  type PermissionMode,
  type PermissionRequest,
  type SessionChat,
  sessionTitle,
  type ToolPart,
  visibleMessages,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Badge,
  Button,
  ChatIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  CloseIcon,
  CopyIcon,
  GitIcon,
  GlobeIcon,
  IconButton,
  PaperclipIcon,
  RefreshIcon,
  Select,
  Spinner,
  UndoIcon,
  useToast,
} from "@zandor/ui";
import {
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { Markdown } from "./Markdown";
import { ModelPicker } from "./ModelPicker";
import { useChat, useEngineState, useProjects, useServices } from "./services";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function useErrorToast() {
  const toast = useToast();
  const { t } = useI18n();
  return (e: unknown) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
}

export function ChatPanel({ empty }: { empty: ReactNode }) {
  const { t } = useI18n();
  const { projects, chat } = useServices();
  const state = useProjects();
  const chatState = useChat();
  const sel = state.selection.sessionId ? projects.selected() : null;
  const sessionId = sel?.session.id ?? null;
  const directory = sel?.session.directory ?? null;

  // Buka sesi terpilih (memuat riwayat + berlangganan SSE); lepas saat berganti.
  useEffect(() => {
    if (!sessionId || !directory || !state.engineReady) return;
    chat.open(directory, sessionId);
    return () => chat.close(sessionId);
  }, [chat, sessionId, directory, state.engineReady]);

  if (!sel) return <>{empty}</>;
  const session = chatState.sessions[sel.session.id];
  const title = sessionTitle(sel.session) ?? t("sb.untitled");

  return (
    <div className="chat">
      <SessionTabs />
      <header className="chat__head">
        <ChatIcon size="sm" />
        <div className="chat__titles">
          <h2 className="z-h4 chat__title">{title}</h2>
          <p className="z-small chat__sub">
            {sel.project ? t("chat.inProject", { name: sel.project.name }) : t("chat.noProject")}
          </p>
        </div>
        <ModelPicker />
      </header>
      {session ? <Transcript session={session} /> : <p className="sidebar__note z-small">{t("chat.loading")}</p>}
      {session && <Composer session={session} />}
    </div>
  );
}

/** Tab sesi yang terbuka (urutan dibuka). Hanya tampil bila ada lebih dari satu. */
function SessionTabs() {
  const { t } = useI18n();
  const { projects } = useServices();
  const state = useProjects();
  if (state.tabs.length < 2) return null;
  const titleOf = (key: string, id: string) => {
    const s = state.sessions[key]?.items.find((i) => i.id === id);
    return s ? (sessionTitle(s) ?? t("sb.untitled")) : t("sb.untitled");
  };
  return (
    <nav className="stabs" aria-label={t("chat.tabs")}>
      {state.tabs.map((tab) => {
        const title = titleOf(tab.key, tab.sessionId);
        const active = state.selection.sessionId === tab.sessionId;
        return (
          <div key={tab.sessionId} className="stabs__tab" data-active={active ? "" : undefined}>
            <button
              type="button"
              className="stabs__label"
              aria-current={active ? "true" : undefined}
              onClick={() => projects.select(tab.key, tab.sessionId)}
            >
              {title}
            </button>
            <button
              type="button"
              className="stabs__close"
              aria-label={t("chat.closeTab", { title })}
              onClick={() => projects.closeTab(tab.sessionId)}
            >
              <CloseIcon size="sm" />
            </button>
          </div>
        );
      })}
    </nav>
  );
}

/** Auto-scroll ke bawah hanya bila pengguna sudah di dekat dasar (tidak merebut scroll saat membaca ke atas). */
/** Di bawah ambang ini semua pesan dirender langsung; di atasnya transcript divirtualisasi (hanya yang terlihat). */
export const VIRTUALIZE_FROM = 40;

function useStickToBottom(dep: unknown) {
  const ref = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `dep` adalah pemicu (isi transcript berubah), bukan nilai yang dibaca
  useLayoutEffect(() => {
    const el = ref.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [dep]);
  const onScroll = () => {
    const el = ref.current;
    if (el) stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
  };
  return { ref, onScroll };
}

function Transcript({ session }: { session: SessionChat }) {
  const { t } = useI18n();
  const { chat } = useServices();
  const fail = useErrorToast();
  const busy = session.status.type !== "idle" || session.sending;
  const messages = visibleMessages(session);
  const lastAssistant = [...messages].reverse().find((m) => m.info.role === "assistant");
  const virtual = messages.length >= VIRTUALIZE_FROM;
  const scrollRef = useRef<HTMLDivElement>(null);
  const rows = useVirtualizer({
    count: virtual ? messages.length : 0,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 140,
    overscan: 6,
    getItemKey: (i) => messages[i]?.info.id ?? i,
  });
  const total = virtual ? rows.getTotalSize() : 0;
  const { ref, onScroll } = useStickToBottom([session.messages, total, session.permissions.length, busy]);
  const setRefs = (el: HTMLDivElement | null) => {
    ref.current = el;
    scrollRef.current = el;
  };

  return (
    <div
      className="chat__scroll"
      ref={setRefs}
      onScroll={onScroll}
      role="log"
      aria-label={t("panel.chat")}
      aria-busy={busy}
    >
      {session.loading && session.messages.length === 0 && <p className="chat__note z-small">{t("chat.loading")}</p>}
      {session.loaded && messages.length === 0 && !session.loading && (
        <p className="chat__note z-small">{t("chat.empty")}</p>
      )}
      {virtual ? (
        <div className="vlist" style={{ height: total }}>
          {rows.getVirtualItems().map((item) => {
            const m = messages[item.index];
            if (!m) return null;
            return (
              <div
                key={item.key}
                className="vrow"
                data-index={item.index}
                ref={rows.measureElement}
                style={{ transform: `translateY(${item.start}px)` }}
              >
                <Message message={m} sessionId={session.sessionID} isLastAssistant={m === lastAssistant && !busy} />
              </div>
            );
          })}
        </div>
      ) : (
        messages.map((m) => (
          <Message
            key={m.info.id}
            message={m}
            sessionId={session.sessionID}
            isLastAssistant={m === lastAssistant && !busy}
          />
        ))
      )}
      {session.revert !== null && (
        <div className="chat__note chat__note--warn z-small" role="status">
          <span>{t("chat.reverted")}</span>
          <Button size="sm" onClick={() => void chat.unrevert(session.sessionID).catch(fail)}>
            {t("chat.unrevert")}
          </Button>
        </div>
      )}
      {session.permissions.map((p) => (
        <PermissionCard key={p.id} request={p} sessionId={session.sessionID} />
      ))}
      {busy && (
        <p className="chat__note z-small" role="status">
          <Spinner size="sm" />{" "}
          {session.status.type === "retry"
            ? t("chat.retry", { attempt: session.status.attempt, message: session.status.message })
            : t("chat.working")}
        </p>
      )}
      {session.reconnecting && <p className="chat__note chat__note--warn z-small">{t("chat.reconnecting")}</p>}
      {session.error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {session.error}
        </p>
      )}
    </div>
  );
}

function MessageActions({
  message,
  sessionId,
  isLastAssistant,
}: {
  message: ChatMessage;
  sessionId: string;
  isLastAssistant: boolean;
}) {
  const { t } = useI18n();
  const { chat, projects } = useServices();
  const state = useProjects();
  const toast = useToast();
  const fail = useErrorToast();
  const text = messageText(message);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      toast.show({ title: t("chat.copied"), tone: "success", timeoutMs: 1500 });
    } catch (e) {
      fail(e);
    }
  };
  const fork = async () => {
    try {
      const s = await chat.fork(sessionId, message.info.id);
      const key = state.selection.key;
      if (key) projects.adoptSession(key, s);
    } catch (e) {
      fail(e);
    }
  };
  return (
    <div className="msg__actions">
      {text && <IconButton label={t("chat.copy")} size="sm" icon={<CopyIcon />} onClick={() => void copy()} />}
      <IconButton label={t("chat.fork")} size="sm" icon={<GitIcon />} onClick={() => void fork()} />
      {message.info.role === "user" && (
        <IconButton
          label={t("chat.revert")}
          size="sm"
          icon={<UndoIcon />}
          onClick={() => void chat.revertTo(sessionId, message.info.id).catch(fail)}
        />
      )}
      {isLastAssistant && (
        <IconButton
          label={t("chat.regenerate")}
          size="sm"
          icon={<RefreshIcon />}
          onClick={() => void chat.regenerate(sessionId).catch(fail)}
        />
      )}
    </div>
  );
}

function Message({
  message,
  sessionId,
  isLastAssistant,
}: {
  message: ChatMessage;
  sessionId: string;
  isLastAssistant: boolean;
}) {
  const { t } = useI18n();
  const { info } = message;
  const visible = message.parts.filter((p) => p.type !== "other" && p.type !== "step-finish");
  if (info.role === "user") {
    const text = messageText(message);
    const files = message.parts.filter((p) => p.type === "file");
    if (!text && files.length === 0) return null;
    return (
      <article className="msg msg--user" aria-label={t("chat.you")}>
        {files.length > 0 && (
          <ul className="chips">
            {files.map((f) =>
              f.type === "file" ? (
                <li key={f.id} className="chip">
                  <PaperclipIcon size="sm" />
                  <span>{f.filename ?? f.url.split("/").pop()}</span>
                </li>
              ) : null,
            )}
          </ul>
        )}
        {text && <p className="msg__text">{text}</p>}
        <MessageActions message={message} sessionId={sessionId} isLastAssistant={false} />
      </article>
    );
  }
  return (
    <article className="msg msg--assistant" aria-label={t("chat.assistant")}>
      {visible.map((p) => (
        <Part key={p.id} part={p} />
      ))}
      {info.error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {info.error.name === "MessageAbortedError" ? t("chat.aborted") : info.error.message}
        </p>
      )}
      <MessageMeta message={message} />
      {info.time.completed !== undefined && (
        <MessageActions message={message} sessionId={sessionId} isLastAssistant={isLastAssistant} />
      )}
    </article>
  );
}

function MessageMeta({ message }: { message: ChatMessage }) {
  const { t } = useI18n();
  const { info } = message;
  if (!info.time.completed || !info.modelID) return null;
  const secs = Math.max(0, (info.time.completed - info.time.created) / 1000);
  const tokens = info.tokens;
  const tip = [
    new Date(info.time.completed).toLocaleString(),
    tokens ? t("chat.meta.tokens", { input: tokens.input, output: tokens.output }) : "",
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <p className="msg__meta z-small" title={tip}>
      {info.modelID} · {secs.toFixed(1)}s
    </p>
  );
}

function Part({ part }: { part: MessagePart }) {
  const { t } = useI18n();
  if (part.type === "text") return part.synthetic || !part.text ? null : <Markdown text={part.text} />;
  if (part.type === "reasoning") return part.text ? <Reasoning text={part.text} label={t("chat.thinking")} /> : null;
  if (part.type === "tool") return <ToolCard part={part} />;
  return null;
}

function Reasoning({ text, label }: { text: string; label: string }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="fold">
      <button type="button" className="fold__head" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        {open ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
        <span>{label}</span>
      </button>
      {open && <p className="fold__body z-small">{text}</p>}
    </div>
  );
}

const WEB_TOOLS = new Set(["websearch", "webfetch", "codesearch"]);
const str = (v: unknown): string => (typeof v === "string" ? v : "");

/** Ringkasan satu baris per jenis tool (pencarian web → kueri/URL; shell → perintah; file → path; dst). */
export function toolSummary(tool: string, input: Record<string, unknown>, title?: string): string {
  const pick = (...keys: string[]) => keys.map((k) => str(input[k])).find(Boolean) ?? "";
  if (tool === "websearch" || tool === "codesearch") return pick("query") || title || "";
  if (tool === "webfetch") return pick("url") || title || "";
  if (tool === "bash") return pick("command", "description") || title || "";
  if (tool === "glob" || tool === "grep") return pick("pattern") || title || "";
  const fallback = pick("filePath", "path", "pattern", "command", "url", "query");
  return title || fallback || (Object.values(input).find((v) => typeof v === "string") as string | undefined) || "";
}

function DiffView({ text }: { text: string }) {
  const lines = text.split("\n");
  return (
    <pre className="fold__pre diff">
      {lines.map((l, i) => {
        const kind =
          l.startsWith("+") && !l.startsWith("+++") ? "add" : l.startsWith("-") && !l.startsWith("---") ? "del" : "ctx";
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: baris diff statis, urutan tetap
          <span key={i} className={`diff__line diff__line--${kind}`}>
            {l}
            {"\n"}
          </span>
        );
      })}
    </pre>
  );
}

function ToolCard({ part }: { part: ToolPart }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const { state } = part;
  const tone = state.status === "error" ? "danger" : state.status === "completed" ? "success" : "outline";
  const body = state.status === "error" ? (state.error ?? "") : (state.output ?? "");
  const diff = typeof state.metadata?.diff === "string" ? state.metadata.diff : "";
  const web = WEB_TOOLS.has(part.tool);
  return (
    <div className="fold fold--tool" data-status={state.status}>
      <button type="button" className="fold__head" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        {open ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
        {web && <GlobeIcon size="sm" />}
        <code className="fold__tool">{web ? t("chat.webSearch") : part.tool}</code>
        <span className="fold__summary z-small">{toolSummary(part.tool, state.input, state.title)}</span>
        <Badge tone={tone}>{t(`chat.tool.${state.status}` as never)}</Badge>
      </button>
      {open && (
        <div className="fold__body">
          {diff && (
            <>
              <p className="z-small fold__label">{t("chat.diff")}</p>
              <DiffView text={diff} />
            </>
          )}
          <p className="z-small fold__label">{t("chat.input")}</p>
          <pre className="fold__pre">{JSON.stringify(state.input, null, 2)}</pre>
          {body && (
            <>
              <p className="z-small fold__label">{t("chat.output")}</p>
              <pre className="fold__pre">{body.length > 20000 ? `${body.slice(0, 20000)}\n…` : body}</pre>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function PermissionCard({ request, sessionId }: { request: PermissionRequest; sessionId: string }) {
  const { t } = useI18n();
  const { chat } = useServices();
  const fail = useErrorToast();
  const [busy, setBusy] = useState(false);
  const answer = async (reply: "once" | "always" | "reject") => {
    setBusy(true);
    try {
      await chat.reply(sessionId, request.id, reply);
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <section
      className="perm"
      role="alertdialog"
      aria-label={t("chat.permission.title", { permission: request.permission })}
    >
      <h3 className="z-label">{t("chat.permission.title", { permission: request.permission })}</h3>
      {request.patterns.length > 0 && <pre className="fold__pre">{request.patterns.join("\n")}</pre>}
      {typeof request.metadata.diff === "string" && <DiffView text={request.metadata.diff} />}
      <div className="perm__actions">
        <Button size="sm" variant="solid" disabled={busy} onClick={() => void answer("once")}>
          {t("chat.permission.once")}
        </Button>
        {request.always.length > 0 && (
          <Button size="sm" disabled={busy} onClick={() => void answer("always")}>
            {t("chat.permission.always")}
          </Button>
        )}
        <Button size="sm" variant="danger" disabled={busy} onClick={() => void answer("reject")}>
          {t("chat.permission.reject")}
        </Button>
      </div>
    </section>
  );
}

function Composer({ session }: { session: SessionChat }) {
  const { t } = useI18n();
  const { chat, host, nav } = useServices();
  const engine = useEngineState();
  const { model, models, modelsStatus, variant, permissionMode } = useChat();
  const toast = useToast();
  const [text, setText] = useState("");
  const [files, setFiles] = useState<FileAttachment[]>([]);
  const running = session.status.type !== "idle" || session.sending;
  const noModel = modelsStatus === "ready" && !model;
  const disabled = engine.status !== "ready";
  const variants =
    models.find((m) => m.providerID === model?.providerID && m.modelID === model?.modelID)?.variants ?? [];
  const canSend = !disabled && !!model && (text.trim() !== "" || files.length > 0);

  const submit = async (e?: FormEvent) => {
    e?.preventDefault();
    if (!canSend || running) return;
    const value = text;
    const attached = files;
    setText("");
    setFiles([]);
    try {
      await chat.send(session.sessionID, value, attached);
    } catch (err) {
      setText(value); // kembalikan agar tidak hilang
      setFiles(attached);
      toast.show({ title: t("chat.sendError", { error: errText(err) }), tone: "danger" });
    }
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      void submit();
    }
  };
  const attach = async (kind: "files" | "folder") => {
    try {
      const picked = await host.pickAttachments(kind);
      if (!picked) return;
      for (const name of picked.skipped) toast.show({ title: t("chat.attachTooLarge", { name }), tone: "warning" });
      setFiles((prev) => [...prev, ...picked.items.filter((n) => !prev.some((p) => p.url === n.url))]);
    } catch (err) {
      toast.show({ title: t("sb.error", { error: errText(err) }), tone: "danger" });
    }
  };

  return (
    <form className="composer" onSubmit={(e) => void submit(e)}>
      {(noModel || (modelsStatus === "ready" && models.length === 0)) && (
        <p className="chat__note chat__note--warn z-small">
          {t("chat.noModel")}{" "}
          <Button size="sm" onClick={() => nav.push("/providers")}>
            {t("pv.manage")}
          </Button>
        </p>
      )}
      {disabled && <p className="chat__note chat__note--warn z-small">{t("chat.engineDown")}</p>}
      {files.length > 0 && (
        <ul className="chips">
          {files.map((f) => (
            <li key={f.url} className="chip">
              <PaperclipIcon size="sm" />
              <span>{f.filename ?? f.url}</span>
              <button
                type="button"
                className="chip__x"
                aria-label={t("chat.removeAttachment", { name: f.filename ?? f.url })}
                onClick={() => setFiles((prev) => prev.filter((p) => p.url !== f.url))}
              >
                <CloseIcon size="sm" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <textarea
        className="z-input composer__input"
        rows={3}
        value={text}
        placeholder={t("chat.placeholder")}
        aria-label={t("chat.placeholder")}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKey}
        disabled={disabled}
      />
      <div className="composer__actions">
        <IconButton
          label={t("chat.attach")}
          size="sm"
          icon={<PaperclipIcon />}
          disabled={disabled}
          onClick={() => void attach("files")}
        />
        {host.native && (
          <Button size="sm" disabled={disabled} onClick={() => void attach("folder")}>
            {t("chat.attachFolder")}
          </Button>
        )}
        <Select
          aria-label={t("chat.permissionMode")}
          size="sm"
          className="composer__mode"
          items={(["ask", "auto-edit", "auto-all"] as PermissionMode[]).map((m) => ({
            value: m,
            label: t(`chat.mode.${m}` as never),
          }))}
          value={permissionMode}
          onValueChange={(v) => chat.setPermissionMode(v as PermissionMode)}
        />
        {variants.length > 0 && (
          <Select
            aria-label={t("chat.variant")}
            size="sm"
            className="composer__mode"
            items={[{ value: "", label: t("chat.variant.default") }, ...variants.map((v) => ({ value: v, label: v }))]}
            value={variant ?? ""}
            onValueChange={(v) => chat.setVariant(v || null)}
          />
        )}
        <span className="composer__spacer" />
        {running ? (
          <Button variant="danger" onClick={() => void chat.abort(session.sessionID)}>
            {t("chat.stop")}
          </Button>
        ) : (
          <Button variant="solid" type="submit" disabled={!canSend}>
            {t("chat.send")}
          </Button>
        )}
      </div>
    </form>
  );
}
