// Dok antrean pesan: pesan yang ditulis selagi agent bekerja menumpuk di sini (di atas composer), dikirim berurutan begitu
// agent selesai. Urutan bisa diubah (seret-lepas atau Alt+↑/↓ pada gagang), pesan bisa diedit, dihapus, atau dikirim langsung.
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowUpIcon,
  Button,
  CheckIcon,
  CloseIcon,
  EditIcon,
  GripIcon,
  IconButton,
  Textarea,
  TrashIcon,
  useToast,
} from "@zandor/ui";
import { type KeyboardEvent, useState } from "react";
import { useChat, useServices } from "./services";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function QueueDock({ sessionId }: { sessionId: string }) {
  const { t } = useI18n();
  const { chat } = useServices();
  const toast = useToast();
  const state = useChat();
  const items = state.queues[sessionId] ?? [];
  const paused = state.queuePaused[sessionId] === true;
  const [editing, setEditing] = useState<{ id: string; value: string } | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [over, setOver] = useState<number | null>(null);
  if (items.length === 0) return null;

  const saveEdit = () => {
    if (editing) chat.editQueued(sessionId, editing.id, editing.value);
    setEditing(null);
  };
  const onGripKey = (e: KeyboardEvent, id: string, index: number) => {
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    chat.moveQueued(sessionId, id, index + (e.key === "ArrowUp" ? -1 : 1));
  };
  const sendNow = async (id: string) => {
    try {
      await chat.sendQueuedNow(sessionId, id);
    } catch (err) {
      toast.show({ title: t("sb.error", { error: errText(err) }), tone: "danger" });
    }
  };

  return (
    <section className="qdock" aria-label={t("queue.title", { n: items.length })}>
      <header className="qdock__head">
        <h3 className="qdock__title">{t("queue.title", { n: items.length })}</h3>
        {paused ? (
          <>
            <span className="z-small qdock__note">{t("queue.paused")}</span>
            <Button size="sm" variant="solid" onClick={() => chat.resumeQueue(sessionId)}>
              {t("queue.resume")}
            </Button>
          </>
        ) : (
          <span className="z-small qdock__note">{t("queue.hint")}</span>
        )}
      </header>
      <ol className="qdock__list">
        {items.map((q, i) => (
          <li
            key={q.id}
            className="qdock__item"
            data-dragging={dragId === q.id ? "" : undefined}
            data-drop={dragId && dragId !== q.id && over === i ? "" : undefined}
            draggable={editing?.id !== q.id}
            onDragStart={(e) => {
              setDragId(q.id);
              e.dataTransfer.effectAllowed = "move";
              e.dataTransfer.setData("text/plain", q.id);
            }}
            onDragOver={(e) => {
              if (!dragId) return;
              e.preventDefault();
              if (over !== i) setOver(i);
            }}
            onDrop={(e) => {
              e.preventDefault();
              if (dragId) chat.moveQueued(sessionId, dragId, i);
              setDragId(null);
              setOver(null);
            }}
            onDragEnd={() => {
              setDragId(null);
              setOver(null);
            }}
          >
            <button
              type="button"
              className="qdock__grip"
              aria-label={t("queue.drag")}
              title={t("queue.drag")}
              onKeyDown={(e) => onGripKey(e, q.id, i)}
            >
              <GripIcon size="sm" />
            </button>
            <span className="z-small qdock__n" aria-hidden="true">
              {i + 1}
            </span>
            {editing?.id === q.id ? (
              <>
                <Textarea
                  className="qdock__edit"
                  rows={2}
                  autoFocus
                  aria-label={t("queue.edit")}
                  value={editing.value}
                  onChange={(e) => setEditing({ id: q.id, value: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                      e.preventDefault();
                      saveEdit();
                    } else if (e.key === "Escape") setEditing(null);
                  }}
                />
                <IconButton size="sm" label={t("queue.save")} icon={<CheckIcon size="sm" />} onClick={saveEdit} />
                <IconButton
                  size="sm"
                  label={t("queue.cancel")}
                  icon={<CloseIcon size="sm" />}
                  onClick={() => setEditing(null)}
                />
              </>
            ) : (
              <>
                <span className="qdock__text">
                  {q.text}
                  {q.files.length > 0 && (
                    <span className="z-small qdock__files"> · {t("queue.files", { n: q.files.length })}</span>
                  )}
                </span>
                <span className="qdock__actions">
                  <IconButton
                    size="sm"
                    label={t("queue.edit")}
                    icon={<EditIcon size="sm" />}
                    onClick={() => setEditing({ id: q.id, value: q.text })}
                  />
                  <IconButton
                    size="sm"
                    label={t("queue.delete")}
                    icon={<TrashIcon size="sm" />}
                    onClick={() => chat.removeQueued(sessionId, q.id)}
                  />
                  <IconButton
                    size="sm"
                    label={t("queue.sendNow")}
                    icon={<ArrowUpIcon size="sm" />}
                    onClick={() => void sendNow(q.id)}
                  />
                </span>
              </>
            )}
          </li>
        ))}
      </ol>
    </section>
  );
}
