import { Toast as BaseToast } from "@base-ui/react/toast";
import { useI18n } from "@zandor/i18n/react";
import type { ReactNode } from "react";
import { cx } from "../cx";
import { BadgeCheckIcon, CloseIcon, InfoIcon, OctagonXIcon, TriangleAlertIcon } from "../icons";

const TONE_ICON = { neutral: InfoIcon, success: BadgeCheckIcon, danger: OctagonXIcon, warning: TriangleAlertIcon };

export type ToastTone = "neutral" | "success" | "danger" | "warning";
export type ToastInput = {
  title: string;
  description?: string;
  tone?: ToastTone;
  timeoutMs?: number;
  /** Tombol aksi di toast (mis. "Buka sesi"). */
  action?: { label: string; onClick: () => void };
};

function ToastList() {
  const { t } = useI18n();
  const { toasts } = BaseToast.useToastManager();
  return toasts.map((toast) => {
    const tone = (toast.type ?? "neutral") as ToastTone;
    const Icon = TONE_ICON[tone] ?? InfoIcon;
    return (
      <BaseToast.Root key={toast.id} toast={toast} className={cx("z-toast", `z-toast--${tone}`)}>
        <Icon size="md" className="z-toast__icon" aria-hidden />
        <BaseToast.Content className="z-toast__content">
          <BaseToast.Title className="z-toast__title" />
          <BaseToast.Description className="z-toast__desc" />
          <BaseToast.Action className="z-toast__action" />
        </BaseToast.Content>
        <BaseToast.Close className="z-toast__close" aria-label={t("ui.close")}>
          <CloseIcon size="sm" />
        </BaseToast.Close>
      </BaseToast.Root>
    );
  });
}

/** Pasang sekali di akar aplikasi. */
export function ToastProvider({ children }: { children: ReactNode }) {
  const { t } = useI18n();
  return (
    <BaseToast.Provider>
      {children}
      <BaseToast.Portal>
        <BaseToast.Viewport className="z-toast-viewport" aria-label={t("ui.notifications")} data-shortcut-scope="toast">
          <ToastList />
        </BaseToast.Viewport>
      </BaseToast.Portal>
    </BaseToast.Provider>
  );
}

export function useToast() {
  const manager = BaseToast.useToastManager();
  return {
    show: (input: ToastInput) =>
      manager.add({
        title: input.title,
        description: input.description,
        type: input.tone ?? "neutral",
        timeout: input.timeoutMs,
        ...(input.action ? { actionProps: { children: input.action.label, onClick: input.action.onClick } } : {}),
      }),
    close: (id?: string) => manager.close(id),
  };
}
