// Aksi per pesan (baris bawah): salin tautan pesan, salin isi, simpan sebagai (teks/gambar/SVG), simpan ke catatan, sematkan,
// buat cabang baru, dan ulangi jawaban. Aksi yang bergantung pada engine (cabang, ulangi, revert) tidak ada di sesi demo.
import {
  FILE_GLOBAL,
  messageLink,
  messageText,
  pinFile,
  projectFile,
  projectKey,
  scanSecrets,
  type TurnMessage,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  BookmarkIcon,
  CodeIcon,
  CopyIcon,
  FileTextIcon,
  GitIcon,
  IconButton,
  ImageDownIcon,
  ImageIcon,
  LinkIcon,
  Menu,
  type MenuEntry,
  PinIcon,
  Popover,
  RefreshIcon,
  UndoIcon,
  useToast,
} from "@zandor/ui";
import { type RefObject, useState } from "react";
import { useDemo } from "./demo-context";
import { type ExportKind, exportFileName, renderNode, textBytes } from "./message-export";
import { useChat, useProjects, useServices } from "./services";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function MessageToolbar({
  message,
  sessionId,
  isLastAssistant,
  answerRef,
}: {
  message: TurnMessage;
  sessionId: string;
  isLastAssistant: boolean;
  /** Elemen jawaban untuk ekspor gambar/SVG (hanya pesan asisten). */
  answerRef?: RefObject<HTMLElement | null>;
}) {
  const { t } = useI18n();
  const { chat, projects, memory, docHost } = useServices();
  const state = useProjects();
  const chatState = useChat();
  const demo = useDemo();
  const toast = useToast();
  const [saveOpen, setSaveOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const text = messageText(message);
  const isAssistant = message.info.role === "assistant";
  const pinnedIds = chatState.pinnedMessages[sessionId] ?? [];
  const pinned = (message.sourceIds ?? [message.info.id]).some((id) => pinnedIds.includes(id));
  const fail = (e: unknown) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
  const copyText = async (value: string, okKey: "chat.copied" | "msg.linkCopied") => {
    try {
      await navigator.clipboard.writeText(value);
      toast.show({ title: t(okKey), tone: "success", timeoutMs: 1500 });
    } catch (e) {
      fail(e);
    }
  };

  const link = () =>
    copyText(
      messageLink({ key: state.selection.key ?? "chat", sessionId, messageId: message.info.id }),
      "msg.linkCopied",
    );

  const saveAs = async (kind: ExportKind) => {
    setBusy(true);
    try {
      let bytes: Uint8Array;
      if (kind === "text") bytes = textBytes(text);
      else {
        const node = answerRef?.current;
        if (!node) throw new Error(t("msg.noContent"));
        bytes = await renderNode(node, kind);
      }
      if (await docHost.saveBytes(exportFileName(kind), bytes)) {
        setSaveOpen(false);
        toast.show({ title: t("msg.saved"), tone: "success", timeoutMs: 2000 });
      }
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const saveNote = async (scope: "project" | "global" | "session") => {
    try {
      if (!text.trim()) throw new Error(t("msg.noContent"));
      if (scanSecrets(text).length > 0) throw new Error(t("msg.noteSecret"));
      const project = projects.selected()?.project ?? null;
      let rel = FILE_GLOBAL;
      if (scope === "project") {
        if (!project) throw new Error(t("msg.noProject"));
        rel = projectFile(await projectKey(project.path));
      } else if (scope === "session") rel = pinFile(sessionId);
      const existing = await memory.readFile(rel).catch(() => "");
      const stamp = new Date().toISOString().slice(0, 16).replace("T", " ");
      await memory.saveFile(rel, `${existing.replace(/\s+$/, "")}${existing ? "\n\n" : ""}## ${stamp}\n\n${text}\n`);
      toast.show({ title: t("msg.noteSaved", { scope: t(`msg.scope.${scope}` as never) }), tone: "success" });
    } catch (e) {
      fail(e);
    }
  };

  const noteEntries: MenuEntry[] = [
    ...(projects.selected()?.project
      ? [
          {
            type: "item" as const,
            id: "project",
            label: t("msg.scope.project"),
            onSelect: () => void saveNote("project"),
          },
        ]
      : []),
    { type: "item", id: "global", label: t("msg.scope.global"), onSelect: () => void saveNote("global") },
    { type: "item", id: "session", label: t("msg.scope.session"), onSelect: () => void saveNote("session") },
  ];

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
    <div className="msg__actions" data-no-export="">
      {isAssistant && (
        <IconButton label={t("msg.copyLink")} size="sm" icon={<LinkIcon />} onClick={() => void link()} />
      )}
      {text && (
        <IconButton
          label={t("chat.copy")}
          size="sm"
          icon={<CopyIcon />}
          onClick={() => void copyText(text, "chat.copied")}
        />
      )}
      {isAssistant && text && (
        <Popover
          open={saveOpen}
          onOpenChange={setSaveOpen}
          trigger={<IconButton label={t("msg.saveAs")} size="sm" icon={<ImageDownIcon />} />}
          title={t("msg.saveAs")}
          side="top"
          align="end"
          className="msave"
        >
          <ul className="msave__list">
            {(
              [
                ["text", t("msg.saveText"), <FileTextIcon key="t" size="sm" />],
                ["png", t("msg.saveImage"), <ImageIcon key="p" size="sm" />],
                ["svg", t("msg.saveSvg"), <CodeIcon key="s" size="sm" />],
              ] as const
            ).map(([kind, label, icon]) => (
              <li key={kind}>
                <button type="button" className="msave__item" disabled={busy} onClick={() => void saveAs(kind)}>
                  {icon}
                  <span>{label}</span>
                </button>
              </li>
            ))}
          </ul>
        </Popover>
      )}
      {isAssistant && text && (
        <Menu
          trigger={<IconButton label={t("msg.saveNote")} size="sm" icon={<BookmarkIcon />} />}
          entries={noteEntries}
        />
      )}
      {isAssistant && (
        <IconButton
          label={pinned ? t("msg.unpin") : t("msg.pin")}
          size="sm"
          icon={<PinIcon />}
          aria-pressed={pinned}
          onClick={() => chat.toggleMessagePin(sessionId, message.info.id)}
        />
      )}
      {!demo && isAssistant && (
        <IconButton label={t("chat.fork")} size="sm" icon={<GitIcon />} onClick={() => void fork()} />
      )}
      {!demo && message.info.role === "user" && (
        <IconButton
          label={t("chat.revert")}
          size="sm"
          icon={<UndoIcon />}
          onClick={() => void chat.revertTo(sessionId, message.info.id).catch(fail)}
        />
      )}
      {!demo && isLastAssistant && (
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
