// Menjalankan penjadwal automasi selama aplikasi terbuka: memasang runner dan mendetak tiap 30 detik. Tidak merender apa pun.
import { useI18n } from "@zandor/i18n/react";
import { useToast } from "@zandor/ui";
import { useEffect, useRef } from "react";
import { createAutomationRunner } from "./automation-runner";
import { useServices } from "./services";

const TICK_MS = 30_000;
const FIRST_TICK_MS = 4_000; // kejar tugas yang terlewat tak lama setelah aplikasi dibuka (menunggu engine siap)

export function AutomationDriver() {
  const { automation, projects, chat, notify } = useServices();
  const { t } = useI18n();
  const toast = useToast();
  const tRef = useRef(t);
  tRef.current = t;
  const toastRef = useRef(toast);
  toastRef.current = toast;

  useEffect(() => {
    automation.setRunner(
      createAutomationRunner({
        projects,
        chat,
        notify,
        toast: (text, tone) => toastRef.current.show({ title: text, tone, timeoutMs: 6000 }),
        msg: () => ({
          title: (name) => tRef.current("auto.notif.title", { name }),
          started: (name) => tRef.current("auto.notif.started", { name }),
          finished: (name) => tRef.current("auto.notif.finished", { name }),
          needsPermission: (name) => tRef.current("auto.notif.needsPermission", { name }),
          failed: (name, error) => tRef.current("auto.notif.failed", { name, error }),
        }),
      }),
    );
    const first = setTimeout(() => void automation.tick(), FIRST_TICK_MS);
    const id = setInterval(() => void automation.tick(), TICK_MS);
    return () => {
      clearTimeout(first);
      clearInterval(id);
      automation.setRunner(null);
    };
  }, [automation, projects, chat, notify]);

  return null;
}
