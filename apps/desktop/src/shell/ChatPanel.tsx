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
  PERMISSION_MODES,
  type PermissionMode,
  type PermissionRequest,
  parseSlash,
  permissionRisk,
  type QuestionRequest,
  type SessionChat,
  type Suggestion,
  sessionTitle,
  suggestPrompts,
  type Todo,
  visibleMessages,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowUpIcon,
  Button,
  ChatIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  CloseIcon,
  ContextMenu,
  Dialog,
  FileTextIcon,
  FolderIcon,
  GitIcon,
  IconButton,
  MaximizeIcon,
  Menu,
  type MenuEntry,
  MicIcon,
  MinimizeIcon,
  ModelLogo,
  PaperclipIcon,
  PinIcon,
  PlusIcon,
  RefreshIcon,
  Select,
  ShieldIcon,
  Spinner,
  StopIcon,
  UndoIcon,
  useToast,
} from "@zandor/ui";
import {
  type FormEvent,
  type KeyboardEvent,
  type ClipboardEvent as ReactClipboardEvent,
  type DragEvent as ReactDragEvent,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { AssistantMessage } from "./AssistantMessage";
import { AttachmentPreview } from "./AttachmentPreview";
import { ChatCustomize } from "./ChatCustomize";
import { ComposerInfo } from "./ComposerInfo";
import { ComposerMemory, type PendingMemory } from "./ComposerMemory";
import { ComposerSuggest } from "./ComposerSuggest";
import { useComposerAssist } from "./composer-assist";
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
import { useOpenState } from "./open-state";
import { PanelTabBar } from "./PanelTabBar";
import { isLongPaste, isPasted, lineCount, pastedAttachment, pastedText } from "./pasted-text";
import { toolSummary } from "./process-label";
import { QuestionCard, TodoList } from "./QuestionCard";
import { QueueDock } from "./QueueDock";
import { TabStatus } from "./SessionDot";
import {
  useAppearance,
  useChat,
  useDocs,
  useEngineState,
  useGit,
  usePanelState,
  useProjects,
  useServices,
  useSettings,
} from "./services";
import { SessionFlagsContext } from "./session-context";
import { useSmoothCaret } from "./smooth-caret";
import { DiffBlock, ToolCard } from "./ToolStep";
import { parseDiff } from "./tool-view";
import { dayPart, useOsUser } from "./use-os-user";
import { loadVoiceDevice, saveVoiceDevice, speechLang, useAudioInputs, useDictation } from "./voice";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));
const PREVIEWABLE_IMAGE_MIMES = new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]);
const isPreviewableImage = (file: FileAttachment) => PREVIEWABLE_IMAGE_MIMES.has(file.mime.toLowerCase());

function useErrorToast() {
  const toast = useToast();
  const { t } = useI18n();
  return (e: unknown) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
}

export function ChatPanel() {
  const { t } = useI18n();
  const { projects, chat } = useServices();
  const { panels } = useServices();
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
        <SessionTabs closePanel={() => panels.actions?.close("chat")} />
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
        <SessionTabs closePanel={() => panels.actions?.close("chat")} />
        <h2 className="z-visually-hidden">{title}</h2>
        {welcome ? (
          <div className="chat__welcome">
            <Greeting />
            <SessionScope />
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
  const { panels } = useServices();
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
      <SessionTabs closePanel={() => panels.actions?.close("chat")} />
      <div className="chat__welcome">
        <Greeting />
        <SessionScope />
        <Composer session={stub} draft />
      </div>
    </div>
  );
}

/** Sapaan di sesi baru: bagian hari + nama pengguna OS (tanpa nama bila tidak diketahui). */
function Greeting() {
  const { t } = useI18n();
  const { profile } = useAppearance();
  const osUser = useOsUser();
  const user = profile.name ?? osUser;
  const part = dayPart(new Date().getHours());
  const name = user ? `, ${user}` : "";
  return (
    <div className="chat__greeting">
      <h3 className="z-h2">{t(`chat.greet.${part}` as never, { name })}</h3>
      <p className="z-p chat__sub">{t("chat.greet.sub")}</p>
    </div>
  );
}

/**
 * Lingkup sesi baru: di project mana (dan repo git + cabangnya) atau chat biasa dengan folder sendiri.
 * Data git dari `GitStore` bersama (sama dengan panel Git); tanpa Tauri ditandai tidak tersedia, bukan ditebak.
 */
function SessionScope() {
  const { t } = useI18n();
  const { git, projects } = useServices();
  const proj = useProjects();
  const g = useGit();
  const toast = useToast();
  const project = proj.projects.find((p) => p.id === (proj.selection.key ?? CHAT_KEY)) ?? null;
  const root = project?.path ?? null;
  // biome-ignore lint/correctness/useExhaustiveDependencies: `git` stabil; hanya root yang relevan
  useEffect(() => {
    if (root) void git.setRoot(root);
  }, [root]);
  // ganti folder = buka draf sesi baru di sana (sesi yang sudah ada terikat ke foldernya)
  const folderEntries: MenuEntry[] = [
    {
      type: "checkbox",
      id: CHAT_KEY,
      label: t("scope.chat"),
      checked: !project,
      onCheckedChange: () => projects.select(CHAT_KEY, null),
    },
    ...proj.projects.map((p) => ({
      type: "checkbox" as const,
      id: p.id,
      label: p.name,
      checked: p.id === project?.id,
      onCheckedChange: () => projects.select(p.id, null),
    })),
  ];
  const folderPicker = (
    <Menu
      trigger={
        <Button
          size="sm"
          variant="ghost"
          className="chat__scope-btn"
          aria-label={t("scope.pickFolder")}
          title={project?.path ?? t("scope.chatHint")}
        >
          {project ? <FolderIcon size="sm" /> : <ChatIcon size="sm" />}
          <span className="chat__scope-name">{project?.name ?? t("scope.chat")}</span>
          <ChevronDownIcon size="sm" />
        </Button>
      }
      entries={folderEntries}
    />
  );
  if (!project) {
    return (
      <div className="chat__scope z-small">
        {folderPicker}
        <span className="chat__scope-meta">{t("scope.chatHint")}</span>
      </div>
    );
  }
  const status = g.projectRoot === root ? g.status : null;
  const gitText = !git.available
    ? t("scope.gitUnavailable")
    : g.loading && !status
      ? t("scope.gitLoading")
      : status?.isRepo
        ? (status.branch ?? t("scope.detached"))
        : t("scope.noGit");
  const switchTo = (name: string) =>
    void git
      .switchBranch(name)
      .catch((e: unknown) =>
        toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" }),
      );
  const branchEntries: MenuEntry[] = g.branches.map((b) => ({
    type: "checkbox" as const,
    id: b.name,
    label: b.name,
    checked: b.current,
    onCheckedChange: () => {
      if (!b.current) switchTo(b.name);
    },
  }));
  return (
    <div className="chat__scope z-small">
      {folderPicker}
      {status?.isRepo && branchEntries.length > 0 ? (
        <Menu
          trigger={
            <Button
              size="sm"
              variant="ghost"
              className="chat__scope-btn"
              aria-label={t("scope.pickBranch")}
              disabled={g.busy}
            >
              <GitIcon size="sm" />
              <span className="chat__scope-meta">{gitText}</span>
              <ChevronDownIcon size="sm" />
            </Button>
          }
          entries={branchEntries}
        />
      ) : (
        <span className="chat__scope-meta">
          <GitIcon size="sm" />
          {gitText}
        </span>
      )}
    </div>
  );
}

/** Tab sesi yang terbuka (urutan dibuka). Hanya tampil bila ada lebih dari satu. */
function SessionTabs({ closePanel }: { closePanel: () => void }) {
  const { t } = useI18n();
  const { projects } = useServices();
  const state = useProjects();
  const chatState = useChat();
  const panelState = usePanelState();
  const barRef = useRef<HTMLElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: dijalankan saat tab aktif berubah agar tab itu terlihat
  useEffect(() => {
    barRef.current
      ?.querySelector<HTMLElement>('[aria-current="true"]')
      ?.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [state.selection.sessionId, state.tabs.length]);
  if (state.tabs.length < 1) {
    return panelState.collapsed.chat ? null : (
      <PanelTabBar closeLabel={t("panel.close.chat")} onClose={closePanel} actions={<ChatCustomize />} />
    );
  }
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
    <PanelTabBar
      closeLabel={t("panel.close.chat")}
      onClose={panelState.collapsed.chat ? undefined : closePanel}
      actions={panelState.collapsed.chat ? undefined : <ChatCustomize />}
    >
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
    </PanelTabBar>
  );
}

/** Auto-scroll ke bawah hanya bila pengguna sudah di dekat dasar (tidak merebut scroll saat membaca ke atas). */
/** Di bawah ambang ini semua pesan dirender langsung; di atasnya transcript divirtualisasi (hanya yang terlihat). */
export const VIRTUALIZE_FROM = 40;

/** Apakah posisi gulir berada di dekat dasar (ambang 48 px). */
const nearBottom = (el: HTMLElement) => el.scrollHeight - el.scrollTop - el.clientHeight < 48;

function useStickToBottom(dep: unknown, follow = true) {
  const ref = useRef<HTMLDivElement>(null);
  const stick = useRef(follow);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `dep` adalah pemicu (isi transcript berubah), bukan nilai yang dibaca
  useLayoutEffect(() => {
    const el = ref.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [dep]);
  // Isi bisa tumbuh tanpa perubahan data (langkah proses dibuka/ditambah, teks mengalir, gambar/diagram selesai dimuat):
  // amati DOM supaya posisi tetap mengikuti ke bawah selama pengguna memang berada di dasar.
  useEffect(() => {
    const el = ref.current;
    if (!el || !follow) return;
    let raf = 0;
    const toBottom = () => {
      raf = 0;
      if (stick.current) el.scrollTop = el.scrollHeight;
    };
    const schedule = () => {
      if (!raf) raf = requestAnimationFrame(toBottom);
    };
    const observer = new MutationObserver(schedule);
    observer.observe(el, { childList: true, subtree: true, characterData: true });
    // klik pengguna (mis. membuka lipatan) memperbesar isi tanpa scroll: hitung ulang apakah masih di dasar
    const onClick = () => setTimeout(() => (stick.current = nearBottom(el)), 0);
    el.addEventListener("click", onClick);
    el.addEventListener("load", schedule, true);
    return () => {
      observer.disconnect();
      el.removeEventListener("click", onClick);
      el.removeEventListener("load", schedule, true);
      if (raf) cancelAnimationFrame(raf);
    };
  }, [follow]);
  const onScroll = () => {
    const el = ref.current;
    if (el && follow) stick.current = nearBottom(el);
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

/**
 * Kunci React satu giliran asisten. Pesan gabungan memakai ID pesan engine TERAKHIR sebagai `info.id`, yang berganti tiap
 * pesan engine baru bergabung; kunci harus mengikuti pesan PERTAMA agar giliran tidak dipasang ulang (state lipatan hilang).
 */
export const turnKey = (m: ChatMessage & { sourceIds?: string[] }): string => m.sourceIds?.[0] ?? m.info.id;

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
    getItemKey: (i) => (messages[i] ? turnKey(messages[i]) : i),
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
            key={turnKey(m)}
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
  const { t, locale } = useI18n();
  const { info } = message;
  if (info.role === "user") {
    const text = messageText(message);
    const files = message.parts.filter((p) => p.type === "file");
    if (!text && files.length === 0) return null;
    const created = new Date(info.time.created);
    const createdAt = created.toLocaleString(locale === "id" ? "id-ID" : "en-US", {
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
    const fullCreatedAt = created.toLocaleString(locale === "id" ? "id-ID" : "en-US", {
      dateStyle: "full",
      timeStyle: "long",
    });
    return (
      <article className="msg msg--user" aria-label={t("chat.you")} id={messageDomId(info.id)}>
        <div className="msg__bubble">
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
        </div>
        <footer className="msg__user-foot">
          <time className="msg__user-time z-small" dateTime={created.toISOString()} title={fullCreatedAt}>
            {createdAt}
          </time>
          <MessageToolbar message={message} sessionId={sessionId} isLastAssistant={false} />
        </footer>
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
  if (part.type === "reasoning")
    return part.text ? <Reasoning id={part.id} text={part.text} label={t("chat.thinking")} /> : null;
  if (part.type === "tool") return <ToolCard part={part} />;
  return null;
}

function Reasoning({ id, text, label }: { id: string; text: string; label: string }) {
  const [open, setOpen] = useOpenState(`think:${id}`);
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

function PermissionCard({ request, sessionId }: { request: PermissionRequest; sessionId: string }) {
  const { t } = useI18n();
  const { chat } = useServices();
  const demo = useDemo();
  const fail = useErrorToast();
  const { permissionMode } = useChat();
  const [busy, setBusy] = useState(false);
  // Di mode "auto-safe" kartu hanya muncul untuk aksi berisiko: jelaskan mengapa ditanyakan.
  const risk = permissionMode === "auto-safe" ? permissionRisk(request) : null;
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
      {risk && <p className="perm__reason z-small">{t(`chat.risk.${risk}` as never)}</p>}
      {request.patterns.length > 0 && <pre className="fold__pre">{request.patterns.join("\n")}</pre>}
      {typeof request.metadata.diff === "string" && <DiffBlock rows={parseDiff(request.metadata.diff)} />}
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
  const { sendKey, pasteLongText } = useSettings();
  const [text, setText] = useState("");
  const [files, setFiles] = useState<FileAttachment[]>([]);
  const [previewFile, setPreviewFile] = useState<FileAttachment | null>(null);
  const [previewZoom, setPreviewZoom] = useState(1);
  const [previewOffset, setPreviewOffset] = useState({ x: 0, y: 0 });
  const [previewDragging, setPreviewDragging] = useState(false);
  const previewDragOrigin = useRef<{ x: number; y: number; offsetX: number; offsetY: number } | null>(null);
  const [dropActive, setDropActive] = useState(false);
  const running = session.status.type !== "idle" || session.sending;
  const noModel = modelsStatus === "ready" && !model;
  const disabled = engine.status !== "ready";
  const variants =
    models.find((m) => m.providerID === model?.providerID && m.modelID === model?.modelID)?.variants ?? [];
  const canSend = !disabled && !!model && (text.trim() !== "" || files.length > 0);
  // saran dihitung selama sesi kosong; saat mengetik hanya disembunyikan (ruangnya tetap) agar composer tidak bergeser
  const suggestions = useSuggestions(session.loaded && session.messages.length === 0 && !running);
  const suggestionsHidden = text !== "";
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const caret = useSmoothCaret(inputRef);
  const plainPaste = useRef(false);
  // tinggi composer mengikuti isi sampai batas CSS (max-height), lalu menggulir di dalam
  // biome-ignore lint/correctness/useExhaustiveDependencies: diukur ulang setiap teks berubah
  useLayoutEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [text]);
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

  /** Halaman bawaan: buat sesi di project terpilih (atau Chat) dan terapkan pilihan memori/skill tertunda. */
  const ensureSession = async (): Promise<string> => {
    const created = await projects.createSession(pstate.selection.key ?? CHAT_KEY);
    chat.open(created.directory, created.id);
    for (const kind of ["memory", "skill"] as const) {
      const on = pendingMemory[kind];
      if (on !== undefined && !projectPath) void memory.setScoped(kind, on, created.id, null).catch(() => {});
    }
    setPendingMemory({});
    return created.id;
  };
  const assist = useComposerAssist({
    session,
    draft,
    root: draft ? projectPath : session.directory || projectPath,
    text,
    setText,
    files,
    setFiles,
    inputRef,
    running,
    disabled,
    ensureSession,
  });

  /**
   * Siap ketik: fokus ke textarea tanpa merebut fokus dari tempat lain yang sedang dipakai (editor dokumen, terminal,
   * dialog/menu terbuka). Fokus tidak dipindah bila pengguna sedang di kolom ketik lain di luar panel chat.
   */
  const focusInput = () => {
    requestAnimationFrame(() => {
      const el = inputRef.current;
      if (!el || el.disabled) return;
      const active = document.activeElement as HTMLElement | null;
      const typing =
        !!active && (active.matches("input, textarea, select, [contenteditable]") || active.isContentEditable);
      const free = !typing || !!active?.closest(".chat");
      // Hanya overlay modal/menu yang menahan fokus; toast memakai role="dialog" non-modal dan tidak boleh menghalangi.
      const blocked = document.querySelector(
        '[role="dialog"]:not([aria-modal="false"]), [role="alertdialog"]:not([aria-modal="false"]), [role="menu"]',
      );
      if (free && !blocked) el.focus({ preventScroll: true });
    });
  };
  // sesi baru/tab berganti, engine baru siap, atau agent selesai bekerja → composer siap ketik
  // biome-ignore lint/correctness/useExhaustiveDependencies: dipicu oleh perubahan sesi, kesiapan, dan status kerja
  useEffect(focusInput, [session.sessionID, draft, disabled, running]);

  const submit = async (e?: FormEvent) => {
    e?.preventDefault();
    if (!canSend) return;
    focusInput(); // tetap siap ketik setelah kirim (mis. kirim dengan klik tombol)
    const slash = parseSlash(text);
    if (slash && (await assist.runSlash(slash, files))) return;
    // "//teks" = kirim garis miring apa adanya
    const value = text.trimStart().startsWith("//") ? text.trimStart().slice(1) : text;
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
      // halaman bawaan: buat sesi di project terpilih (atau Chat) lalu kirim pesan pertama
      if (draft) sessionId = await ensureSession();
      await chat.send(sessionId, value, attached);
    } catch (err) {
      setText(value); // kembalikan agar tidak hilang
      setFiles(attached);
      toast.show({ title: t("chat.sendError", { error: errText(err) }), tone: "danger" });
    }
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    plainPaste.current = (e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "v";
    if (assist.onKeyDown(e)) return;
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
      for (const name of picked.skipped)
        toast.show({ title: t("chat.attachTooLarge", { name, error: t("chat.readFailure") }), tone: "warning" });
      setFiles((prev) => [...prev, ...picked.items.filter((n) => !prev.some((p) => p.url === n.url))]);
    } catch (err) {
      toast.show({ title: t("sb.error", { error: errText(err) }), tone: "danger" });
    }
  };
  const addBrowserFiles = async (incoming: File[]) => {
    if (incoming.length === 0) return;
    try {
      const picked = await host.attachmentsFromFiles(incoming);
      for (const name of picked.skipped)
        toast.show({ title: t("chat.attachTooLarge", { name, error: t("chat.readFailure") }), tone: "warning" });
      if (picked.items.length > 0) {
        setFiles((prev) => [
          ...prev,
          ...picked.items.filter((item) => !prev.some((existing) => existing.url === item.url)),
        ]);
      }
    } catch (err) {
      toast.show({ title: t("chat.attachReadError", { error: errText(err) }), tone: "danger" });
    }
  };
  const closePreview = () => {
    setPreviewFile(null);
    setPreviewZoom(1);
    setPreviewOffset({ x: 0, y: 0 });
    setPreviewDragging(false);
    previewDragOrigin.current = null;
  };
  const changePreviewZoom = (delta: number) => setPreviewZoom((zoom) => Math.min(5, Math.max(0.5, zoom + delta)));
  const onPaste = (e: ReactClipboardEvent<HTMLTextAreaElement>) => {
    const plain = plainPaste.current;
    plainPaste.current = false;
    const clipboardFiles = Array.from(e.clipboardData.files);
    if (clipboardFiles.length > 0) {
      e.preventDefault();
      void addBrowserFiles(clipboardFiles);
      return;
    }
    // teks panjang → lampiran teks (composer tetap rapi); Ctrl+Shift+V = tempel apa adanya
    const pasted = e.clipboardData.getData("text/plain");
    if (plain || pasteLongText !== "attach" || !isLongPaste(pasted)) return;
    e.preventDefault();
    setFiles((prev) => [...prev, pastedAttachment(pasted, prev)]);
    toast.show({ title: t("chat.pastedNotice") });
  };
  /** Lampiran teks ditempel → kembali jadi teks biasa di posisi kursor. */
  const inlinePasted = (file: FileAttachment) => {
    const content = pastedText(file);
    if (content === null) return;
    const el = inputRef.current;
    const at = el ? el.selectionStart : text.length;
    const end = el ? el.selectionEnd : text.length;
    setText(text.slice(0, at) + content + text.slice(end));
    setFiles((prev) => prev.filter((p) => p.url !== file.url));
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(at + content.length, at + content.length);
    });
  };
  const onDragOver = (e: ReactDragEvent<HTMLFormElement>) => {
    if (!Array.from(e.dataTransfer.types).includes("Files")) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    setDropActive(true);
  };
  const onDragLeave = (e: ReactDragEvent<HTMLFormElement>) => {
    if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
    setDropActive(false);
  };
  const onDrop = (e: ReactDragEvent<HTMLFormElement>) => {
    const dropped = Array.from(e.dataTransfer.files);
    if (dropped.length === 0) return;
    e.preventDefault();
    setDropActive(false);
    void addBrowserFiles(dropped);
  };

  return (
    <form
      className="composer"
      onSubmit={(e) => void submit(e)}
      onDragEnter={onDragOver}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      data-drop-active={dropActive ? "" : undefined}
    >
      {dropActive && <div className="composer__drop-hint">{t("chat.dropFiles")}</div>}
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
          {files.map((f) => {
            const pastedLines = isPasted(f) ? lineCount(pastedText(f) ?? "") : null;
            return pastedLines !== null ? (
              <li key={f.url} className="chip chip--pasted">
                <button
                  type="button"
                  className="chip__preview"
                  aria-label={t("chat.pastedPreview", { n: pastedLines })}
                  onClick={() => setPreviewFile(f)}
                >
                  <FileTextIcon size="sm" />
                </button>
                <span>{t("chat.pastedText", { n: pastedLines })}</span>
                <IconButton
                  className="chip__x"
                  size="sm"
                  icon={<UndoIcon size="sm" />}
                  label={t("chat.pastedToInline")}
                  onClick={() => inlinePasted(f)}
                />
                <button
                  type="button"
                  className="chip__x"
                  aria-label={t("chat.removeAttachment", { name: t("chat.pastedText", { n: pastedLines }) })}
                  onClick={() => setFiles((prev) => prev.filter((p) => p.url !== f.url))}
                >
                  <CloseIcon size="sm" />
                </button>
              </li>
            ) : (
              <li key={f.url} className="chip" data-image={isPreviewableImage(f) ? "" : undefined}>
                {f.url.startsWith("data:") ? (
                  <button
                    type="button"
                    className="chip__preview"
                    aria-label={t(isPreviewableImage(f) ? "chat.previewAttachment" : "chat.previewFile", {
                      name: f.filename ?? f.url,
                    })}
                    onClick={() => setPreviewFile(f)}
                  >
                    {isPreviewableImage(f) ? <img src={f.url} alt="" /> : <PaperclipIcon size="sm" />}
                  </button>
                ) : (
                  <PaperclipIcon size="sm" />
                )}
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
            );
          })}
          <li className="chip chip--clear-all">
            <IconButton
              className="chip__x"
              size="sm"
              icon={<CloseIcon size="sm" />}
              label={t("chat.removeAllAttachments", { n: files.length })}
              onClick={() => setFiles([])}
            />
          </li>
        </ul>
      )}
      {previewFile && isPreviewableImage(previewFile) ? (
        <Dialog
          open
          onOpenChange={(open) => !open && closePreview()}
          title={previewFile.filename ?? t("chat.imagePreview")}
          className="composer__preview-dialog"
          footer={
            <div className="composer__preview-controls">
              <IconButton
                label={t("chat.zoomOut")}
                size="sm"
                icon={<MinimizeIcon />}
                disabled={previewZoom <= 0.5}
                onClick={() => changePreviewZoom(-0.25)}
              />
              <span className="z-small composer__preview-zoom">{Math.round(previewZoom * 100)}%</span>
              <IconButton
                label={t("chat.zoomIn")}
                size="sm"
                icon={<MaximizeIcon />}
                disabled={previewZoom >= 5}
                onClick={() => changePreviewZoom(0.25)}
              />
              <IconButton
                label={t("chat.zoomReset")}
                size="sm"
                icon={<RefreshIcon />}
                disabled={previewZoom === 1 && previewOffset.x === 0 && previewOffset.y === 0}
                onClick={() => {
                  setPreviewZoom(1);
                  setPreviewOffset({ x: 0, y: 0 });
                }}
              />
              <Button size="sm" onClick={closePreview}>
                {t("ui.close")}
              </Button>
            </div>
          }
        >
          <div
            className="composer__preview-stage"
            role="application"
            aria-label={t("chat.imagePreview")}
            data-dragging={previewDragging ? "" : undefined}
            onDragStart={(e) => e.preventDefault()}
            onWheel={(e) => {
              e.preventDefault();
              changePreviewZoom(e.deltaY < 0 ? 0.1 : -0.1);
            }}
            onPointerDown={(e) => {
              if (e.button !== 0) return;
              e.preventDefault();
              e.currentTarget.setPointerCapture(e.pointerId);
              previewDragOrigin.current = {
                x: e.clientX,
                y: e.clientY,
                offsetX: previewOffset.x,
                offsetY: previewOffset.y,
              };
              setPreviewDragging(true);
            }}
            onPointerMove={(e) => {
              const origin = previewDragOrigin.current;
              if (!origin) return;
              setPreviewOffset({ x: origin.offsetX + e.clientX - origin.x, y: origin.offsetY + e.clientY - origin.y });
            }}
            onPointerUp={() => {
              previewDragOrigin.current = null;
              setPreviewDragging(false);
            }}
            onPointerCancel={() => {
              previewDragOrigin.current = null;
              setPreviewDragging(false);
            }}
            onLostPointerCapture={() => {
              previewDragOrigin.current = null;
              setPreviewDragging(false);
            }}
          >
            <img
              className="composer__preview-image"
              src={previewFile.url}
              alt={previewFile.filename ?? t("chat.imagePreview")}
              draggable={false}
              style={{ transform: `translate(${previewOffset.x}px, ${previewOffset.y}px) scale(${previewZoom})` }}
            />
          </div>
        </Dialog>
      ) : (
        <AttachmentPreview
          file={previewFile}
          open={previewFile !== null}
          onOpenChange={(open) => !open && closePreview()}
        />
      )}
      <div className="composer__card">
        {assist.popup && <ComposerSuggest {...assist.popup} />}
        <div className="composer__field">
          <textarea
            ref={inputRef}
            className="composer__input"
            rows={2}
            value={text}
            placeholder={t("chat.placeholder")}
            aria-label={t("chat.placeholder")}
            aria-controls={assist.popup?.id}
            aria-activedescendant={assist.activeId}
            aria-autocomplete="list"
            onChange={(e) => {
              setText(e.target.value);
              assist.trackCaret(e.target);
            }}
            onSelect={(e) => assist.trackCaret(e.currentTarget)}
            onKeyDown={onKey}
            onPaste={onPaste}
            disabled={disabled}
          />
          <span
            aria-hidden="true"
            className="composer__caret"
            data-visible={caret.visible || undefined}
            data-typing={caret.typing || undefined}
            style={{ transform: `translate(${caret.x}px, ${caret.y}px)`, height: caret.h || undefined }}
          />
        </div>
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
            items={PERMISSION_MODES.map((m) => ({
              value: m,
              label: t(`chat.mode.${m}` as never),
              description: t(`chat.modeDesc.${m}` as never),
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
        <ul
          className="chips suggest"
          aria-label={t("sugg.title")}
          data-hidden={suggestionsHidden ? "" : undefined}
          aria-hidden={suggestionsHidden || undefined}
          inert={suggestionsHidden || undefined}
        >
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
