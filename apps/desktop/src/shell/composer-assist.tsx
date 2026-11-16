// Asisten composer: saran `/perintah` dan `@berkas`, navigasi keyboard, dan eksekusi perintah slash
// (bawaan aplikasi + perintah engine). Dipakai oleh Composer di ChatPanel.
import {
  activeToken,
  BUILTIN_COMMANDS,
  BUILTIN_NEEDS_ARGS,
  type BuiltinCommand,
  CHAT_KEY,
  type ComposerToken,
  type EngineCommand,
  type FileAttachment,
  isBuiltin,
  mentionAttachment,
  messageText,
  type ParsedSlash,
  PERMISSION_MODES,
  type PermissionMode,
  rankByQuery,
  replaceToken,
  type SessionChat,
  type SessionInfo,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  CopyIcon,
  FileIcon,
  GitIcon,
  PlusIcon,
  RefreshIcon,
  ShieldIcon,
  SparklesIcon,
  StopIcon,
  UndoIcon,
  useToast,
  WrenchIcon,
} from "@zandor/ui";
import { type KeyboardEvent, type ReactNode, type RefObject, useEffect, useMemo, useRef, useState } from "react";
import type { SuggestItem } from "./ComposerSuggest";
import { useProjects, useServices } from "./services";

const BUILTIN_ICON: Record<BuiltinCommand, ReactNode> = {
  btw: <SparklesIcon size="sm" />,
  new: <PlusIcon size="sm" />,
  compact: <RefreshIcon size="sm" />,
  undo: <UndoIcon size="sm" />,
  redo: <RefreshIcon size="sm" />,
  fork: <GitIcon size="sm" />,
  copy: <CopyIcon size="sm" />,
  mode: <ShieldIcon size="sm" />,
  stop: <StopIcon size="sm" />,
};

type Entry =
  | { kind: "builtin"; name: BuiltinCommand; item: SuggestItem }
  | { kind: "engine"; name: string; item: SuggestItem }
  | { kind: "file"; path: string; item: SuggestItem };

type Found = { query: string; root: string; paths: string[]; error?: string };

export function useComposerAssist(opts: {
  session: SessionChat;
  draft: boolean;
  /** Folder project/sesi untuk mention dan perintah engine (null = tanpa project di halaman bawaan). */
  root: string | null;
  text: string;
  setText: (v: string) => void;
  files: FileAttachment[];
  setFiles: (f: FileAttachment[] | ((prev: FileAttachment[]) => FileAttachment[])) => void;
  inputRef: RefObject<HTMLTextAreaElement | null>;
  running: boolean;
  disabled: boolean;
  /** Buat sesi untuk halaman bawaan (draft) lalu kembalikan id-nya. */
  ensureSession: () => Promise<string>;
}) {
  const { t } = useI18n();
  const { chat, projects } = useServices();
  const pstate = useProjects();
  const toast = useToast();
  const { session, draft, root, text, setText, setFiles, inputRef, running, disabled } = opts;
  const [caret, setCaret] = useState(0);
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [active, setActive] = useState(0);
  const [commands, setCommands] = useState<{ root: string; list: EngineCommand[] } | null>(null);
  const [found, setFound] = useState<Found | null>(null);
  const commandsReq = useRef<string | null>(null);

  const token: ComposerToken | null = activeToken(text, Math.min(caret, text.length));
  const tokenKey = token ? `${token.kind}:${token.start}` : null;
  const open = token !== null && tokenKey !== dismissed && !disabled;
  const engineList = commands && commands.root === root ? commands.list : [];

  // perintah engine: dimuat sekali per folder saat pertama dibutuhkan (popup slash atau eksekusi)
  const loadCommands = async (): Promise<EngineCommand[]> => {
    if (!root) return [];
    if (commands?.root === root) return commands.list;
    commandsReq.current = root;
    try {
      const list = await chat.listCommands(root);
      if (commandsReq.current === root) setCommands({ root, list });
      return list;
    } catch {
      return [];
    }
  };
  const wantCommands = open && token?.kind === "slash";
  // biome-ignore lint/correctness/useExhaustiveDependencies: loadCommands membaca state terbaru; pemicu cukup root/popup
  useEffect(() => {
    if (wantCommands) void loadCommands();
  }, [wantCommands, root]);

  // mention: cari berkas lewat engine dengan jeda singkat
  const mentionQuery = open && token?.kind === "mention" ? token.query : null;
  useEffect(() => {
    if (mentionQuery === null || !root) return;
    let live = true;
    const timer = setTimeout(() => {
      chat
        .findFiles(root, mentionQuery, 30)
        .then((paths) => live && setFound({ query: mentionQuery, root, paths }))
        .catch((e: unknown) => live && setFound({ query: mentionQuery, root, paths: [], error: String(e) }));
    }, 120);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [mentionQuery, root, chat]);

  const entries: Entry[] = useMemo(() => {
    if (!open || !token) return [];
    if (token.kind === "slash") {
      const builtin = BUILTIN_COMMANDS.map((name) => ({
        kind: "builtin" as const,
        name,
        item: {
          id: `b:${name}`,
          label: `/${name}`,
          ...(BUILTIN_NEEDS_ARGS.includes(name) ? { hint: t(`slash.${name}.args` as never) } : {}),
          description: t(`slash.${name}.desc` as never),
          icon: BUILTIN_ICON[name],
          group: t("slash.title"),
        },
      }));
      const engine = engineList
        .filter((c) => !isBuiltin(c.name))
        .map((c) => ({
          kind: "engine" as const,
          name: c.name,
          item: {
            id: `e:${c.name}`,
            label: `/${c.name}`,
            ...(c.hints.length ? { hint: c.hints.join(" ") } : {}),
            ...(c.description ? { description: c.description } : {}),
            ...(c.source && c.source !== "command" ? { badge: t(`slash.source.${c.source}` as never) } : {}),
            icon: <WrenchIcon size="sm" />,
            group: t("slash.engine"),
          },
        }));
      return [
        ...rankByQuery<Entry & { name: string }>(builtin, token.query, (e) => e.name),
        ...rankByQuery<Entry & { name: string }>(engine, token.query, (e) => e.name),
      ];
    }
    if (!found || found.root !== root || found.query !== token.query) return [];
    return found.paths.map((path) => {
      const clean = path.replace(/\\/g, "/").replace(/\/$/, "");
      const cut = clean.lastIndexOf("/");
      return {
        kind: "file" as const,
        path: clean,
        item: {
          id: `f:${clean}`,
          label: cut >= 0 ? clean.slice(cut + 1) : clean,
          ...(cut >= 0 ? { hint: clean.slice(0, cut) } : {}),
          icon: <FileIcon size="sm" />,
        },
      };
    });
  }, [open, token, engineList, found, root, t]);

  const index = entries.length ? Math.min(active, entries.length - 1) : 0;
  let status: string | null = null;
  if (open && token?.kind === "mention") {
    if (!root) status = t("mention.noRoot");
    else if (found?.error && found.query === token.query) status = t("mention.error", { error: found.error });
    else if (!found || found.query !== token.query) status = t("mention.loading");
    else status = t("mention.empty");
  }
  const visible = open && (entries.length > 0 || status !== null);

  const placeCaret = (pos: number) => {
    setCaret(pos);
    requestAnimationFrame(() => {
      const el = inputRef.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(pos, pos);
    });
  };

  const pick = (i: number) => {
    const entry = entries[i];
    if (!entry || !token) return;
    if (entry.kind === "file") {
      if (!root) return;
      const next = replaceToken(text, token, `@${entry.path}`);
      setText(next.text);
      placeCaret(next.caret);
      const att = mentionAttachment(root, entry.path);
      setFiles((prev) => (prev.some((p) => p.url === att.url) ? prev : [...prev, att]));
    } else {
      const next = replaceToken(text, token, `/${entry.name}`);
      setText(next.text);
      placeCaret(next.caret);
    }
    setActive(0);
  };

  /** Tangani tombol untuk popup. True = sudah ditangani (Composer tidak memprosesnya lagi). */
  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>): boolean => {
    if (visible) {
      if (e.key === "Escape") {
        e.preventDefault();
        setDismissed(tokenKey);
        return true;
      }
      if (entries.length > 0) {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          const step = e.key === "ArrowDown" ? 1 : -1;
          setActive((index + step + entries.length) % entries.length);
          return true;
        }
        if ((e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) || e.key === "Tab") {
          e.preventDefault();
          pick(index);
          return true;
        }
      }
    }
    // ↑ di composer kosong: panggil ulang pesan terakhir yang dikirim
    if (e.key === "ArrowUp" && text === "" && !draft) {
      const last = [...session.messages].reverse().find((m) => m.info.role === "user");
      const prev = last ? messageText(last) : "";
      if (prev) {
        e.preventDefault();
        setText(prev);
        placeCaret(prev.length);
        return true;
      }
    }
    return false;
  };

  const trackCaret = (el: HTMLTextAreaElement) => {
    setCaret(el.selectionStart ?? el.value.length);
    setActive(0);
  };

  const adopt = (s: SessionInfo) => projects.adoptSession(pstate.selection.key ?? CHAT_KEY, s);
  const note = (title: string, tone: "neutral" | "warning" | "danger" | "success" = "neutral") =>
    toast.show({ title, tone });

  /** Jalankan perintah slash. True = sudah ditangani (tidak dikirim sebagai pesan biasa). */
  const runSlash = async ({ name, args }: ParsedSlash, attached: FileAttachment[]): Promise<boolean> => {
    const sid = draft ? null : session.sessionID;
    const clear = () => {
      setText("");
      setFiles([]);
      setCaret(0);
    };
    try {
      if (isBuiltin(name)) {
        if (BUILTIN_NEEDS_ARGS.includes(name) && !args) {
          note(t("slash.needsArgs", { name, args: t(`slash.${name}.args` as never) }), "warning");
          return true;
        }
        if (name === "new") {
          clear();
          await projects.createSession(pstate.selection.key ?? CHAT_KEY);
          return true;
        }
        if (name === "mode") {
          if (!(PERMISSION_MODES as readonly string[]).includes(args)) {
            note(t("slash.badMode", { mode: args }), "warning");
            return true;
          }
          chat.setPermissionMode(args as PermissionMode);
          clear();
          note(t("slash.modeSet", { mode: t(`chat.mode.${args}` as never) }), "success");
          return true;
        }
        if (!sid) {
          note(t("slash.needsSession", { name }), "warning");
          return true;
        }
        if (name === "btw") {
          clear();
          adopt(await chat.aside(sid, args, attached));
          note(t("slash.btwStarted"), "success");
          return true;
        }
        if (name === "stop") {
          clear();
          await chat.abort(sid);
          return true;
        }
        if (name === "fork") {
          clear();
          adopt(await chat.fork(sid));
          return true;
        }
        if (name === "copy") {
          const last = [...session.messages].reverse().find((m) => m.info.role === "assistant");
          const answer = last ? messageText(last) : "";
          if (!answer) {
            note(t("slash.nothing", { name }), "warning");
            return true;
          }
          await navigator.clipboard.writeText(answer);
          clear();
          note(t("slash.copied"), "success");
          return true;
        }
        if (running) {
          note(t("slash.busy", { name }), "warning");
          return true;
        }
        if (name === "compact") {
          if (session.messages.length === 0) {
            note(t("slash.nothing", { name }), "warning");
            return true;
          }
          clear();
          await chat.compact(sid);
          note(t("slash.compacted"), "success");
          return true;
        }
        if (name === "undo") {
          const cut = session.revert;
          const visibleMsgs = cut === null ? session.messages : session.messages.filter((m) => m.info.id < cut);
          const last = [...visibleMsgs].reverse().find((m) => m.info.role === "user");
          if (!last) {
            note(t("slash.nothing", { name }), "warning");
            return true;
          }
          await chat.revertTo(sid, last.info.id);
          const prev = messageText(last);
          setText(prev);
          setFiles([]);
          placeCaret(prev.length);
          return true;
        }
        // redo
        if (session.revert === null) {
          note(t("slash.nothing", { name }), "warning");
          return true;
        }
        clear();
        await chat.unrevert(sid);
        return true;
      }
      const list = await loadCommands();
      if (!list.some((c) => c.name === name)) {
        note(t("slash.unknown", { name }), "warning");
        return true;
      }
      if (running) {
        note(t("slash.busy", { name }), "warning");
        return true;
      }
      clear();
      const target = sid ?? (await opts.ensureSession());
      await chat.runCommand(target, name, args, attached);
      return true;
    } catch (err) {
      note(t("slash.error", { name, error: err instanceof Error ? err.message : String(err) }), "danger");
      return true;
    }
  };

  return {
    popup: visible
      ? {
          id: `csug-${session.sessionID}`,
          title: token?.kind === "slash" ? t("slash.title") : t("mention.title"),
          items: entries.map((e) => e.item),
          active: index,
          status,
          onPick: pick,
          onHover: setActive,
        }
      : null,
    activeId: visible && entries.length ? `csug-${session.sessionID}-${index}` : undefined,
    onKeyDown,
    trackCaret,
    runSlash,
  };
}
