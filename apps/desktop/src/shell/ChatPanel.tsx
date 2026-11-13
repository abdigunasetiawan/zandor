// Panel 2: percakapan. Transcript dari ChatStore (SSE engine), composer, pemilih model, kartu izin, stop.
import {
  type ChatMessage,
  type MessagePart,
  messageText,
  type PermissionRequest,
  type SessionChat,
  sessionTitle,
  type ToolPart,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Badge, Button, ChatIcon, ChevronDownIcon, ChevronRightIcon, Select, Spinner, useToast } from "@zandor/ui";
import { type FormEvent, type KeyboardEvent, useEffect, useLayoutEffect, useRef, useState } from "react";
import { Markdown } from "./Markdown";
import { useChat, useEngineState, useProjects, useServices } from "./services";

export function ChatPanel({ empty }: { empty: React.ReactNode }) {
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

function ModelPicker() {
  const { t } = useI18n();
  const { chat } = useServices();
  const s = useChat();
  if (s.modelsStatus === "error")
    return <Badge tone="danger">{t("chat.modelsError", { error: s.modelsError ?? "" })}</Badge>;
  if (s.models.length === 0) return null;
  const key = (p: string, m: string) => `${p}\u0000${m}`;
  const items = s.models.map((m) => ({
    value: key(m.providerID, m.modelID),
    label: `${m.name}${s.models.some((o) => o.providerID !== m.providerID) ? ` · ${m.providerName}` : ""}`,
  }));
  return (
    <Select
      aria-label={t("chat.model")}
      size="sm"
      className="chat__model"
      items={items}
      value={s.model ? key(s.model.providerID, s.model.modelID) : null}
      onValueChange={(v) => {
        const [providerID, modelID] = v.split("\u0000");
        if (providerID && modelID) chat.selectModel({ providerID, modelID });
      }}
    />
  );
}

/** Auto-scroll ke bawah hanya bila pengguna sudah di dekat dasar (tidak merebut scroll saat membaca ke atas). */
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
  const { ref, onScroll } = useStickToBottom(session.messages);
  const busy = session.status.type !== "idle" || session.sending;
  return (
    <div
      className="chat__scroll"
      ref={ref}
      onScroll={onScroll}
      role="log"
      aria-label={t("panel.chat")}
      aria-busy={busy}
    >
      {session.loading && session.messages.length === 0 && <p className="chat__note z-small">{t("chat.loading")}</p>}
      {session.loaded && session.messages.length === 0 && !session.loading && (
        <p className="chat__note z-small">{t("chat.empty")}</p>
      )}
      {session.messages.map((m) => (
        <Message key={m.info.id} message={m} />
      ))}
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

function Message({ message }: { message: ChatMessage }) {
  const { t } = useI18n();
  const { info } = message;
  const visible = message.parts.filter((p) => p.type !== "other" && p.type !== "step-finish");
  if (info.role === "user") {
    const text = messageText(message);
    if (!text) return null;
    return (
      <article className="msg msg--user" aria-label={t("chat.you")}>
        <p className="msg__text">{text}</p>
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

const summarize = (input: Record<string, unknown>): string => {
  const first = Object.values(input).find((v) => typeof v === "string") as string | undefined;
  return first ?? "";
};

function ToolCard({ part }: { part: ToolPart }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const { state } = part;
  const tone = state.status === "error" ? "danger" : state.status === "completed" ? "success" : "outline";
  const body = state.status === "error" ? (state.error ?? "") : (state.output ?? "");
  return (
    <div className="fold fold--tool" data-status={state.status}>
      <button type="button" className="fold__head" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        {open ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
        <code className="fold__tool">{part.tool}</code>
        <span className="fold__summary z-small">{state.title ?? summarize(state.input)}</span>
        <Badge tone={tone}>{t(`chat.tool.${state.status}` as never)}</Badge>
      </button>
      {open && (
        <div className="fold__body">
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
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const answer = async (reply: "once" | "always" | "reject") => {
    setBusy(true);
    try {
      await chat.reply(sessionId, request.id, reply);
    } catch (e) {
      toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });
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
      {typeof request.metadata.diff === "string" && <pre className="fold__pre perm__diff">{request.metadata.diff}</pre>}
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
  const { chat } = useServices();
  const engine = useEngineState();
  const { model, models, modelsStatus } = useChat();
  const toast = useToast();
  const [text, setText] = useState("");
  const running = session.status.type !== "idle" || session.sending;
  const noModel = modelsStatus === "ready" && !model;
  const disabled = engine.status !== "ready";

  const submit = async (e?: FormEvent) => {
    e?.preventDefault();
    const value = text.trim();
    if (!value || running || disabled || !model) return;
    setText("");
    try {
      await chat.send(session.sessionID, value);
    } catch (err) {
      setText(value); // kembalikan agar tidak hilang
      toast.show({
        title: t("chat.sendError", { error: err instanceof Error ? err.message : String(err) }),
        tone: "danger",
      });
    }
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      void submit();
    }
  };
  return (
    <form className="composer" onSubmit={(e) => void submit(e)}>
      {(noModel || (modelsStatus === "ready" && models.length === 0)) && (
        <p className="chat__note chat__note--warn z-small">{t("chat.noModel")}</p>
      )}
      {disabled && <p className="chat__note chat__note--warn z-small">{t("chat.engineDown")}</p>}
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
        {running ? (
          <Button variant="danger" onClick={() => void chat.abort(session.sessionID)}>
            {t("chat.stop")}
          </Button>
        ) : (
          <Button variant="solid" type="submit" disabled={disabled || !model || !text.trim()}>
            {t("chat.send")}
          </Button>
        )}
      </div>
    </form>
  );
}
