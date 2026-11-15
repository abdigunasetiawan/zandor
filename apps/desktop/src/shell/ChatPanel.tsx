// Panel 2: percakapan. Tab sesi, transcript dari ChatStore (SSE engine), aksi pesan (salin/cabang/revert/ulang),
// composer (lampiran, mode izin, reasoning, model), kartu izin, stop.

import { useVirtualizer } from "@tanstack/react-virtual";
import {
  CHAT_KEY,
  type ChatMessage,
  type FileAttachment,
  initialChatState,
  type MessagePart,
  mergeAssistantTurns,
  messageText,
  type PermissionMode,
  type PermissionRequest,
  type QuestionRequest,
  type SessionChat,
  type Suggestion,
  sessionTitle,
  suggestPrompts,
  type Todo,
  type ToolPart,
  visibleMessages,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowUpIcon,
  Badge,
  Button,
  ChevronDownIcon,
  ChevronRightIcon,
  CloseIcon,
  ContextMenu,
  GlobeIcon,
  IconButton,
  Menu,
  type MenuEntry,
  MicIcon,
  ModelLogo,
  PaperclipIcon,
  PinIcon,
  PlusIcon,
  Select,
  ShieldIcon,
  Spinner,
  StopIcon,
  useToast,
} from "@zandor/ui";
import { type FormEvent, type KeyboardEvent, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { AssistantMessage } from "./AssistantMessage";
import { ComposerInfo } from "./ComposerInfo";
import { ComposerMemory, type PendingMemory } from "./ComposerMemory";
import { DemoChat } from "./DemoChat";
import { useDemo } from "./demo-context";
import { DEMO_KEY, demoById } from "./demo-sessions";
import { Markdown } from "./Markdown";
import { MessageToolbar } from "./MessageToolbar";
import { ModelPicker } from "./ModelPicker";
import {
  clearMessageFocus,
  messageDomId,
  onMessageFocus,
  peekMessageFocus,
  requestMessageFocus,
} from "./message-focus";
import { toolSummary } from "./process-label";
import { QuestionCard, TodoList } from "./QuestionCard";
import { QueueDock } from "./QueueDock";
import { TabStatus } from "./SessionDot";
import {
  useChat,
  useDocs,
  useEngineState,
  useGit,
  useOpenDoc,
  useProjects,
  useServices,
  useSettings,
} from "./services";
import { SessionFlagsContext } from "./session-context";
import { dayPart, useOsUser } from "./use-os-user";
import { loadVoiceDevice, saveVoiceDevice, speechLang, useAudioInputs, useDictation } from "./voice";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function useErrorToast() {
  const toast = useToast();
  const { t } = useI18n();
  return (e: unknown) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
}

export function ChatPanel() {
  const { t } = useI18n();
  const { projects, chat } = useServices();
  const state = useProjects();
  const chatState = useChat();
  const sel = state.selection.sessionId ? projects.selected() : null;
  const sessionId = sel?.session.id ?? null;
  const directory = sel?.session.directory ?? null;

  // Buka sesi terpilih DAN semua sesi di tab terbuka (memuat riwayat + berlangganan SSE), supaya sesi di latar belakang tetap
  // dipantau (indikator status, notifikasi selesai/gagal/butuh aksi). Dilepas saat tabnya ditutup.
  const opened = useRef(new Map<string, string>());
  const wanted = new Map<string, string>();
  if (sessionId && directory) wanted.set(sessionId, directory);
  for (const tab of state.tabs) {
    if (tab.key === DEMO_KEY) continue;
    const dir = state.sessions[tab.key]?.items.find((x) => x.id === tab.sessionId)?.directory;
    if (dir) wanted.set(tab.sessionId, dir);
  }
  const wantedKey = [...wanted].map(([id, dir]) => `${id}@${dir}`).join("|");
  // biome-ignore lint/correctness/useExhaustiveDependencies: `wantedKey` merangkum daftar sesi yang dibutuhkan
  useEffect(() => {
    if (!state.engineReady) {
      for (const id of opened.current.keys()) chat.close(id);
      opened.current.clear();
      return;
    }
    for (const [id, dir] of wanted) {
      if (opened.current.has(id)) continue;
      chat.open(dir, id);
      opened.current.set(id, dir);
    }
    for (const id of [...opened.current.keys()]) {
      if (wanted.has(id)) continue;
      chat.close(id);
      opened.current.delete(id);
    }
  }, [chat, state.engineReady, wantedKey]);
  // sesi yang dipilih lagi harus punya aliran event (folder yang lama tak dipakai dilepas oleh batas aliran)
  useEffect(() => {
    if (state.engineReady && sessionId) chat.resume(sessionId);
  }, [chat, state.engineReady, sessionId]);
  // (sengaja tanpa pelepasan saat panel dilepas: sesi tetap dipantau saat Anda membuka halaman lain, mis. Pengaturan)

  // sesi demo bawaan (statis, hanya-baca): tidak berasal dari engine
  const demoId = state.selection.key === DEMO_KEY ? state.selection.sessionId : null;
  if (demoId && demoById(demoId)) {
    return (
      <div className="chat" data-demo="">
        <SessionTabs />
        <h2 className="z-visually-hidden">{t(demoById(demoId)?.titleKey ?? "demo.trajectory.title")}</h2>
        <DemoChat key={demoId} id={demoId} />
      </div>
    );
  }
  // tanpa sesi terpilih/terbuka: halaman bawaan Panel 2 = seperti sesi baru (sapaan + composer); sesi dibuat saat pesan pertama dikirim
  if (!sel) return <DraftChat />;
  const session = chatState.sessions[sel.session.id];
  const title = sessionTitle(sel.session) ?? t("sb.untitled");

  const welcome = !!session && session.loaded && session.messages.length === 0 && !session.error;

  return (
    <SessionFlagsContext.Provider value={{ hasProject: sel.project !== null }}>
      <div className="chat" data-welcome={welcome ? "" : undefined}>
        <SessionTabs />
        <h2 className="z-visually-hidden">{title}</h2>
        {welcome ? (
          <div className="chat__welcome">
            <Greeting />
            <Composer session={session} />
          </div>
        ) : (
          <>
            {session ? <Transcript session={session} /> : <p className="sidebar__note z-small">{t("chat.loading")}</p>}
            {session && (
              <PromptDock
                sessionId={session.sessionID}
                permissions={session.permissions}
                questions={session.questions}
                todos={session.todos}
              />
            )}
            {session && <Composer session={session} />}
          </>
        )}
      </div>
    </SessionFlagsContext.Provider>
  );
}

/**
 * Dok di atas composer (seperti rencana agent): izin yang diminta, pertanyaan agent, dan rencana tugas. Berada di luar isi
 * respons agar tidak tenggelam di riwayat dan selalu terlihat sampai dijawab.
 */
export function PromptDock({
  sessionId,
  permissions,
  questions,
  todos,
}: {
  sessionId: string;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  todos: Todo[];
}) {
  const { t } = useI18n();
  const pending = permissions.length + questions.length;
  const queued = (useChat().queues[sessionId] ?? []).length;
  if (pending === 0 && todos.length === 0 && queued === 0) return null;
  return (
    <section className="dock" aria-label={t("dock.label")} data-pending={pending > 0 ? "" : undefined}>
      {questions.map((q) => (
        <QuestionCard key={q.id} request={q} sessionId={sessionId} />
      ))}
      {permissions.map((p) => (
        <PermissionCard key={p.id} request={p} sessionId={sessionId} />
      ))}
      <TodoList todos={todos} />
      <QueueDock sessionId={sessionId} />
    </section>
  );
}

/** Halaman bawaan Panel 2 saat tidak ada sesi terbuka: tampilan sesi baru; sesi sungguhan dibuat saat pesan pertama dikirim. */
function DraftChat() {
  const stub = useMemo<SessionChat>(
    () => ({
      ...initialChatState("draft"),
      loaded: true,
      directory: "",
      sending: false,
      loading: false,
      reconnecting: false,
    }),
    [],
  );
  return (
    <div className="chat" data-welcome="" data-draft="">
      <SessionTabs />
      <div className="chat__welcome">
        <Greeting />
        <Composer session={stub} draft />
      </div>
    </div>
  );
}

/** Sapaan di sesi baru: bagian hari + nama pengguna OS (tanpa nama bila tidak diketahui). */
function Greeting() {
  const { t } = useI18n();
  const user = useOsUser();
  const part = dayPart(new Date().getHours());
  const name = user ? `, ${user}` : "";
  return (
    <div className="chat__greeting">
      <h3 className="z-h2">{t(`chat.greet.${part}` as never, { name })}</h3>
      <p className="z-p chat__sub">{t("chat.greet.sub")}</p>
    </div>
  );
}

/** Tab sesi yang terbuka (urutan dibuka). Hanya tampil bila ada lebih dari satu. */
function SessionTabs() {
  const { t } = useI18n();
  const { projects } = useServices();
  const state = useProjects();
  const chatState = useChat();
  const barRef = useRef<HTMLElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: dijalankan saat tab aktif berubah agar tab itu terlihat
  useEffect(() => {
    barRef.current
      ?.querySelector<HTMLElement>('[aria-current="true"]')
      ?.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [state.selection.sessionId, state.tabs.length]);
  if (state.tabs.length < 1) return null;
  // model sesi = model pesan asisten terakhir; bila belum ada, model terpilih saat ini
  const modelOf = (sessionId: string) => {
    const msgs = chatState.sessions[sessionId]?.messages ?? [];
    const last = [...msgs].reverse().find((m) => m.info.role === "assistant" && m.info.modelID);
    return last ? { providerID: last.info.providerID, modelID: last.info.modelID } : chatState.model;
  };
  const titleOf = (key: string, id: string) => {
    if (key === DEMO_KEY) {
      const d = demoById(id);
      return d ? t(d.titleKey) : t("sb.untitled");
    }
    const s = state.sessions[key]?.items.find((i) => i.id === id);
    return s ? (sessionTitle(s) ?? t("sb.untitled")) : t("sb.untitled");
  };
  return (
    <nav className="stabs" aria-label={t("chat.tabs")} ref={barRef}>
      {state.tabs.map((tab, index) => {
        const title = titleOf(tab.key, tab.sessionId);
        const active = state.selection.sessionId === tab.sessionId;
        const last = state.tabs.length - 1;
        const entries: MenuEntry[] = [
          { type: "item", id: "close", label: t("tabs.close"), onSelect: () => projects.closeTab(tab.sessionId) },
          {
            type: "item",
            id: "others",
            label: t("tabs.closeOthers"),
            disabled: state.tabs.length < 2,
            onSelect: () => projects.closeTabsBy("others", tab.sessionId),
          },
          {
            type: "item",
            id: "right",
            label: t("tabs.closeRight"),
            disabled: index === last,
            onSelect: () => projects.closeTabsBy("right", tab.sessionId),
          },
          {
            type: "item",
            id: "left",
            label: t("tabs.closeLeft"),
            disabled: index === 0,
            onSelect: () => projects.closeTabsBy("left", tab.sessionId),
          },
          { type: "separator", id: "sep" },
          { type: "item", id: "all", label: t("tabs.closeAll"), onSelect: () => projects.closeTabsBy("all") },
        ];
        return (
          <ContextMenu key={tab.sessionId} entries={entries} className="stabs__ctx">
            <div className="stabs__tab" data-active={active ? "" : undefined}>
              <button
                type="button"
                className="stabs__label"
                aria-current={active ? "true" : undefined}
                onClick={() => projects.select(tab.key, tab.sessionId)}
              >
                {tab.key === DEMO_KEY ? (
                  <PinIcon size="sm" />
                ) : (
                  <ModelLogo
                    providerID={modelOf(tab.sessionId)?.providerID}
                    modelID={modelOf(tab.sessionId)?.modelID}
                  />
                )}
                {tab.key !== DEMO_KEY && <TabStatus sessionId={tab.sessionId} />}
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
          </ContextMenu>
        );
      })}
    </nav>
  );
}

/** Auto-scroll ke bawah hanya bila pengguna sudah di dekat dasar (tidak merebut scroll saat membaca ke atas). */
/** Di bawah ambang ini semua pesan dirender langsung; di atasnya transcript divirtualisasi (hanya yang terlihat). */
export const VIRTUALIZE_FROM = 40;

function useStickToBottom(dep: unknown, follow = true) {
  const ref = useRef<HTMLDivElement>(null);
  const stick = useRef(follow);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `dep` adalah pemicu (isi transcript berubah), bukan nilai yang dibaca
  useLayoutEffect(() => {
    const el = ref.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [dep]);
  const onScroll = () => {
    const el = ref.current;
    if (el && follow) stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
  };
  return { ref, onScroll };
}

/** Daftar pesan yang disematkan di sesi (di atas transkrip); klik → gulir ke pesan, tombol lepas sematan. */
function PinnedBar({ sessionId, messages }: { sessionId: string; messages: ChatMessage[] }) {
  const { t } = useI18n();
  const { chat } = useServices();
  const { pinnedMessages } = useChat();
  const ids = pinnedMessages[sessionId] ?? [];
  const pinned = ids.map((id) => messages.find((m) => m.info.id === id)).filter((m): m is ChatMessage => !!m);
  if (pinned.length === 0) return null;
  return (
    <details className="pins" data-no-export="">
      <summary className="z-label">
        <PinIcon size="sm" /> {t("msg.pinned", { n: pinned.length })}
      </summary>
      <ul className="pins__list">
        {pinned.map((m) => {
          const excerpt = messageText(m).replace(/\s+/g, " ").trim().slice(0, 90) || t("sb.untitled");
          return (
            <li key={m.info.id} className="pins__row">
              <button type="button" className="pins__go" onClick={() => requestMessageFocus(m.info.id)}>
                <span className="pins__role z-small">
                  {m.info.role === "user" ? t("chat.you") : t("chat.assistant")}
                </span>
                <span className="pins__text">{excerpt}</span>
              </button>
              <IconButton
                label={t("msg.unpin")}
                size="sm"
                className="pins__unpin"
                icon={<PinIcon />}
                aria-pressed
                onClick={() => chat.toggleMessagePin(sessionId, m.info.id)}
              />
            </li>
          );
        })}
      </ul>
    </details>
  );
}

/** `follow=false`: tidak menempel ke dasar (dipakai sesi demo agar dibaca dari atas). */
export function Transcript({ session, follow = true }: { session: SessionChat; follow?: boolean }) {
  const { t } = useI18n();
  const { chat } = useServices();
  const fail = useErrorToast();
  const busy = session.status.type !== "idle" || session.sending;
  const allMessages = visibleMessages(session);
  // pesan asisten berurutan = satu giliran (satu proses, satu jawaban, satu baris info/aksi)
  const messages = useMemo(() => mergeAssistantTurns(allMessages), [allMessages]);
  const lastAssistant = [...messages].reverse().find((m) => m.info.role === "assistant");
  // pesan yang sedang dikerjakan = pesan terakhir percakapan saat engine sibuk (bukan jawaban turn sebelumnya)
  const active = busy && messages.at(-1) === lastAssistant ? lastAssistant : undefined;
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
  const { ref, onScroll } = useStickToBottom(
    [session.messages, total, session.permissions.length, session.questions.length, busy],
    follow,
  );
  // gulir ke pesan yang diminta (tautan pesan / daftar sematan); menunggu sampai elemennya ada
  const visibleCount = rows.getVirtualItems().length;
  // biome-ignore lint/correctness/useExhaustiveDependencies: dijalankan ulang saat isi/virtualisasi berubah agar permintaan tertunda terpenuhi
  useEffect(() => {
    const run = () => {
      const id = peekMessageFocus();
      if (!id) return;
      const index = messages.findIndex((m) => m.sourceIds?.includes(id) ?? m.info.id === id);
      const target = messages[index];
      if (!target) return;
      const el = document.getElementById(messageDomId(target.info.id));
      if (!el) {
        if (virtual) rows.scrollToIndex(index, { align: "center" });
        return;
      }
      clearMessageFocus();
      el.scrollIntoView({ block: "center" });
      el.setAttribute("data-flash", "");
      setTimeout(() => el.removeAttribute("data-flash"), 1600);
    };
    run();
    return onMessageFocus(run);
  }, [messages, virtual, visibleCount]);
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
      <PinnedBar sessionId={session.sessionID} messages={allMessages} />
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
                <Message
                  message={m}
                  sessionId={session.sessionID}
                  isLastAssistant={m === lastAssistant && !busy}
                  streaming={m === active}
                />
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
            streaming={m === active}
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
      {busy && !active && (
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

function Message({
  message,
  sessionId,
  isLastAssistant,
  streaming = false,
}: {
  message: ChatMessage;
  sessionId: string;
  isLastAssistant: boolean;
  streaming?: boolean;
}) {
  const { t } = useI18n();
  const { info } = message;
  if (info.role === "user") {
    const text = messageText(message);
    const files = message.parts.filter((p) => p.type === "file");
    if (!text && files.length === 0) return null;
    return (
      <article className="msg msg--user" aria-label={t("chat.you")} id={messageDomId(info.id)}>
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
        <MessageToolbar message={message} sessionId={sessionId} isLastAssistant={false} />
      </article>
    );
  }
  return (
    <AssistantMessage message={message} sessionId={sessionId} isLastAssistant={isLastAssistant} streaming={streaming} />
  );
}

export function Part({ part, streaming = false }: { part: MessagePart; streaming?: boolean }) {
  const { t } = useI18n();
  if (part.type === "text")
    return part.synthetic || !part.text ? null : <Markdown text={part.text} streaming={streaming} />;
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

export { toolSummary };

const WEB_TOOLS = new Set(["websearch", "webfetch", "codesearch"]);
const FILE_TOOLS = new Set(["read", "edit", "write", "patch", "multiedit"]);
const str = (v: unknown): string => (typeof v === "string" ? v : "");

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
  const openDoc = useOpenDoc();
  const toast = useToast();
  const demo = useDemo();
  // di sesi demo berkasnya fiktif: tombol Buka tidak ditampilkan
  const file = !demo && FILE_TOOLS.has(part.tool) ? str(state.input.filePath) || str(state.input.path) : "";
  const openFile = () => {
    if (!openDoc(file)) toast.show({ title: t("doc.outside"), tone: "warning" });
  };
  return (
    <div className="fold fold--tool" data-status={state.status}>
      <button type="button" className="fold__head" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        {open ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
        {web && <GlobeIcon size="sm" />}
        <code className="fold__tool">{web ? t("chat.webSearch") : part.tool}</code>
        <span className="fold__summary z-small">{toolSummary(part.tool, state.input, state.title)}</span>
        <Badge tone={tone}>{t(`chat.tool.${state.status}` as never)}</Badge>
      </button>
      {file && (
        <Button size="sm" onClick={openFile} aria-label={t("doc.open", { name: file.split(/[\\/]/).at(-1) ?? file })}>
          {t("doc.openShort")}
        </Button>
      )}
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
  const demo = useDemo();
  const fail = useErrorToast();
  const [busy, setBusy] = useState(false);
  const answer = async (reply: "once" | "always" | "reject") => {
    if (demo) return demo.permission(request.id, reply);
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

/** Saran prompt dari konteks nyata: git, berkas yang dibuka, dan isi akar project. */
function useSuggestions(enabled: boolean): Suggestion[] {
  const { projects } = useServices();
  const state = useProjects();
  const git = useGit();
  const docs = useDocs();
  const sel = state.selection.sessionId ? projects.selected() : null;
  const projectPath = sel?.project?.path ?? state.projects.find((p) => p.id === state.selection.key)?.path ?? null;
  const [rootFiles, setRootFiles] = useState<string[] | null>(null);
  const engineReady = state.engineReady;

  useEffect(() => {
    setRootFiles(null);
    const api = projects.engineApi();
    if (!enabled || !projectPath || !api || !engineReady) return;
    let live = true;
    api
      .listFiles(projectPath, ".")
      .then((nodes) => live && setRootFiles(nodes.map((n) => n.name)))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [enabled, projectPath, engineReady, projects]);

  if (!enabled) return [];
  const active = docs.tabs.find((d) => d.id === docs.activeId);
  const files = git.status?.isRepo ? git.status.files : [];
  return suggestPrompts({
    hasProject: projectPath !== null,
    rootFiles,
    changed: files.map((f) => f.path),
    staged: files.filter((f) => f.index !== " " && f.index !== "?").length,
    activeDoc: active && active.root === projectPath ? { path: active.path, kind: active.kind } : null,
  });
}

function Composer({ session, draft = false }: { session: SessionChat; draft?: boolean }) {
  const { t } = useI18n();
  const { chat, host, nav, projects, memory } = useServices();
  const pstate = useProjects();
  const [pendingMemory, setPendingMemory] = useState<PendingMemory>({});
  const projectPath = draft
    ? (pstate.projects.find((p) => p.id === pstate.selection.key)?.path ?? null)
    : (projects.selected()?.project?.path ?? null);
  const engine = useEngineState();
  const { model, models, modelsStatus, variant, permissionMode } = useChat();
  const toast = useToast();
  const { sendKey } = useSettings();
  const [text, setText] = useState("");
  const [files, setFiles] = useState<FileAttachment[]>([]);
  const running = session.status.type !== "idle" || session.sending;
  const noModel = modelsStatus === "ready" && !model;
  const disabled = engine.status !== "ready";
  const variants =
    models.find((m) => m.providerID === model?.providerID && m.modelID === model?.modelID)?.variants ?? [];
  const canSend = !disabled && !!model && (text.trim() !== "" || files.length > 0);
  const showSuggestions = session.loaded && session.messages.length === 0 && text === "" && !running;
  const suggestions = useSuggestions(showSuggestions);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const { locale } = useI18n();
  const [voiceDevice, setVoiceDevice] = useState(loadVoiceDevice);
  const { devices, refresh: refreshDevices } = useAudioInputs();
  const voice = useDictation({
    lang: speechLang(locale),
    deviceId: voiceDevice,
    onText: (chunk) => setText((prev) => (prev && !prev.endsWith(" ") ? `${prev} ${chunk}` : prev + chunk)),
    onError: (error) => toast.show({ title: t("voice.error", { error }), tone: "danger" }),
  });
  const chooseDevice = (id: string) => {
    setVoiceDevice(id);
    saveVoiceDevice(id);
  };
  const deviceEntries: MenuEntry[] = [
    {
      type: "checkbox",
      id: "default",
      label: t("voice.defaultDevice"),
      checked: voiceDevice === "",
      onCheckedChange: () => chooseDevice(""),
    },
    ...devices.map((d, i) => ({
      type: "checkbox" as const,
      id: d.deviceId,
      label: d.label || t("voice.deviceN", { n: i + 1 }),
      checked: voiceDevice === d.deviceId,
      onCheckedChange: () => chooseDevice(d.deviceId),
    })),
    ...(devices.length === 0
      ? [{ type: "item" as const, id: "none", label: t("voice.noDevice"), disabled: true, onSelect: () => {} }]
      : []),
  ];

  const submit = async (e?: FormEvent) => {
    e?.preventDefault();
    if (!canSend) return;
    const value = text;
    const attached = files;
    if (running) {
      // agent sedang bekerja: pesan masuk antrean (dok di atas composer), dikirim berurutan setelah selesai
      try {
        chat.enqueue(session.sessionID, value, attached);
        setText("");
        setFiles([]);
      } catch (err) {
        toast.show({ title: t("chat.sendError", { error: errText(err) }), tone: "danger" });
      }
      return;
    }
    setText("");
    setFiles([]);
    try {
      let sessionId = session.sessionID;
      if (draft) {
        // halaman bawaan: buat sesi di project terpilih (atau Chat) lalu kirim pesan pertama
        const created = await projects.createSession(pstate.selection.key ?? CHAT_KEY);
        chat.open(created.directory, created.id);
        sessionId = created.id;
        // pilihan memori/skill creator untuk sesi Chat baru (di project, pengaturan project yang berlaku)
        for (const kind of ["memory", "skill"] as const) {
          const on = pendingMemory[kind];
          if (on !== undefined && !projectPath) void memory.setScoped(kind, on, created.id, null).catch(() => {});
        }
        setPendingMemory({});
      }
      await chat.send(sessionId, value, attached);
    } catch (err) {
      setText(value); // kembalikan agar tidak hilang
      setFiles(attached);
      toast.show({ title: t("chat.sendError", { error: errText(err) }), tone: "danger" });
    }
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key !== "Enter" || e.nativeEvent.isComposing) return;
    // "enter": Enter kirim, Shift+Enter baris baru. "ctrl-enter": Ctrl/Cmd+Enter kirim, Enter baris baru.
    const send = sendKey === "ctrl-enter" ? e.ctrlKey || e.metaKey : !e.shiftKey;
    if (send) {
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
          <Button size="sm" onClick={() => nav.push("/settings/models")}>
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
      <div className="composer__card">
        <textarea
          ref={inputRef}
          className="composer__input"
          rows={2}
          value={text}
          placeholder={t("chat.placeholder")}
          aria-label={t("chat.placeholder")}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onKey}
          disabled={disabled}
        />
        <div className="composer__actions">
          {host.native ? (
            <Menu
              trigger={
                <IconButton
                  label={t("chat.attach")}
                  size="sm"
                  className="composer__round"
                  icon={<PlusIcon />}
                  disabled={disabled}
                />
              }
              entries={[
                { type: "item", id: "files", label: t("chat.attach"), onSelect: () => void attach("files") },
                { type: "item", id: "folder", label: t("chat.attachFolder"), onSelect: () => void attach("folder") },
              ]}
            />
          ) : (
            <IconButton
              label={t("chat.attach")}
              size="sm"
              className="composer__round"
              icon={<PlusIcon />}
              disabled={disabled}
              onClick={() => void attach("files")}
            />
          )}
          <Select
            aria-label={t("chat.permissionMode")}
            size="sm"
            icon={<ShieldIcon size="sm" />}
            className={permissionMode === "auto-all" ? "composer__mode composer__mode--full" : "composer__mode"}
            items={(["ask", "auto-edit", "auto-all"] as PermissionMode[]).map((m) => ({
              value: m,
              label: t(`chat.mode.${m}` as never),
            }))}
            value={permissionMode}
            onValueChange={(v) => chat.setPermissionMode(v as PermissionMode)}
          />
          <ComposerMemory
            sessionId={draft ? null : session.sessionID}
            projectPath={projectPath}
            pending={pendingMemory}
            onPending={setPendingMemory}
          />
          <span className="composer__spacer" />
          <ComposerInfo session={session} />
          <div className="composer__model">
            <ModelPicker />
            {variants.length > 0 && (
              <Select
                aria-label={t("chat.variant")}
                size="sm"
                className="composer__variant"
                items={[
                  { value: "", label: t("chat.variant.default") },
                  ...variants.map((v) => ({ value: v, label: v })),
                ]}
                value={variant ?? ""}
                onValueChange={(v) => chat.setVariant(v || null)}
              />
            )}
          </div>
          <div className="composer__voice" data-listening={voice.listening ? "" : undefined}>
            <IconButton
              label={voice.listening ? t("voice.stop") : t("voice.start")}
              size="sm"
              icon={voice.listening ? <StopIcon /> : <MicIcon />}
              aria-pressed={voice.listening}
              disabled={disabled || !voice.supported}
              title={voice.supported ? undefined : t("voice.unsupported")}
              onClick={voice.toggle}
            />
            <Menu
              trigger={
                <IconButton
                  label={t("voice.device")}
                  size="sm"
                  className="composer__voice-device"
                  icon={<ChevronDownIcon />}
                  disabled={!voice.supported}
                  onClick={refreshDevices}
                />
              }
              entries={deviceEntries}
            />
          </div>
          {running && canSend && (
            <IconButton
              label={t("queue.add")}
              variant="solid"
              type="submit"
              className="composer__round composer__send"
              icon={<ArrowUpIcon />}
            />
          )}
          {running ? (
            <IconButton
              label={t("chat.stop")}
              variant="danger"
              className="composer__round composer__send"
              icon={<StopIcon />}
              onClick={() => void chat.abort(session.sessionID)}
            />
          ) : (
            <IconButton
              label={t("chat.send")}
              variant="solid"
              type="submit"
              className="composer__round composer__send"
              icon={<ArrowUpIcon />}
              disabled={!canSend}
            />
          )}
        </div>
      </div>
      {suggestions.length > 0 && (
        <ul className="chips suggest" aria-label={t("sugg.title")}>
          {suggestions.map((s) => (
            <li key={s.id}>
              <button
                type="button"
                className="suggest__chip"
                disabled={disabled}
                title={t(`sugg.${s.id}.prompt` as never, s.params)}
                onClick={() => {
                  setText(t(`sugg.${s.id}.prompt` as never, s.params));
                  inputRef.current?.focus();
                }}
              >
                {t(`sugg.${s.id}.label` as never, s.params)}
              </button>
            </li>
          ))}
        </ul>
      )}
    </form>
  );
}
