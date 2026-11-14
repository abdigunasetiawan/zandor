// Penggerak pemberitahuan sesi: memantau state chat, mendeteksi peralihan (selesai, gagal/batas, butuh aksi), lalu memberi tahu
// lewat toast, notifikasi sistem, dan suara sesuai Pengaturan. Tidak tampil apa pun; juga menandai sesi "sudah dilihat".
import { type AttentionEvent, SETTLE_MS, type Settings } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { useToast } from "@zandor/ui";
import { useEffect, useRef } from "react";
import { useServices, useSettings } from "./services";
import { playSound, type SoundEvent } from "./sound";

const SOUND_EVENT: Record<AttentionEvent["type"], SoundEvent> = {
  done: "done",
  failed: "failed",
  limit: "failed",
  action: "action",
};

export function NotificationDriver() {
  const { chat, projects, attention, notify } = useServices();
  const settings = useSettings();
  const toast = useToast();
  const { t } = useI18n();
  // nilai terbaru untuk pemanggil yang berumur panjang (tanpa memasang ulang langganan tiap render)
  const latest = useRef({ settings, toast, t });
  latest.current = { settings, toast, t };

  useEffect(() => {
    let focused = typeof document === "undefined" ? true : document.hasFocus();

    const locate = (sessionId: string): { key: string; title: string } | null => {
      const st = projects.state();
      for (const [key, group] of Object.entries(st.sessions)) {
        const s = group?.items.find((x) => x.id === sessionId);
        if (s) return { key, title: s.title };
      }
      return null;
    };

    const enabled = (ev: AttentionEvent, s: Settings): boolean =>
      ev.type === "done" ? s.notifyDone : ev.type === "action" ? s.notifyAction : s.notifyFailed;

    const handle = (ev: AttentionEvent) => {
      const { settings: s, toast: tst, t: tr } = latest.current;
      if (!enabled(ev, s)) return;
      const viewing = projects.state().selection.sessionId === ev.sessionId;
      const watching = viewing && focused;
      if (s.notifyOnlyAway && watching) return;
      const found = locate(ev.sessionId);
      const name = found?.title && !/^New session - /.test(found.title) ? found.title : tr("sb.untitled");
      let title: string;
      if (ev.type === "action") title = ev.kind === "permission" ? tr("notify.permission") : tr("notify.question");
      else if (ev.type === "done") title = tr("notify.done");
      else title = ev.type === "limit" ? tr("notify.limit") : tr("notify.failed");
      const body =
        ev.type === "done" ? name : ev.type === "action" ? `${name} — ${ev.detail}` : `${name} — ${ev.message}`;

      if (s.notifyToast) {
        tst.show({
          title,
          description: body,
          tone: ev.type === "done" ? "success" : ev.type === "action" ? "warning" : "danger",
          timeoutMs: ev.type === "done" ? 6000 : 10000,
          ...(found
            ? { action: { label: tr("notify.open"), onClick: () => projects.select(found.key, ev.sessionId) } }
            : {}),
        });
      }
      if (s.notifySystem && !focused) void notify.notify(title, body);
      if (s.notifySound) {
        const which = SOUND_EVENT[ev.type];
        const choice = which === "done" ? s.soundDone : which === "failed" ? s.soundFailed : s.soundAction;
        void playSound(choice, s.notifyVolume, which);
      }
    };

    let settleTimer: ReturnType<typeof setTimeout> | undefined;
    const run = () => {
      const events = attention.observe(chat.state().sessions, {
        viewing: projects.state().selection.sessionId,
        focused,
      });
      for (const ev of events) handle(ev);
      // akhir giliran dinilai setelah mengendap: amati lagi sesaat kemudian
      if (attention.hasPending() && !settleTimer) {
        settleTimer = setTimeout(() => {
          settleTimer = undefined;
          run();
        }, SETTLE_MS + 50);
      }
    };
    const onFocus = () => {
      focused = true;
      run();
    };
    const onBlur = () => {
      focused = false;
    };
    const unsubs = [chat.subscribe(run), projects.subscribe(run)];
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    run();
    return () => {
      if (settleTimer) clearTimeout(settleTimer);
      for (const u of unsubs) u();
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, [chat, projects, attention, notify]);

  return null;
}
