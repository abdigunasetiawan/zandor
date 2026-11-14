import { Toast as BaseToast } from "@base-ui/react/toast";
import { useI18n } from "@zandor/i18n/react";
import type { ReactNode } from "react";
import { cx } from "../cx";
import { CloseIcon } from "../icons";

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
  return toasts.map((toast) => (
    <BaseToast.Root key={toast.id} toast={toast} className={cx("z-toast", `z-toast--${toast.type ?? "neutral"}`)}>
      <BaseToast.Content className="z-toast__content">
        <BaseToast.Title className="z-label" />
        <BaseToast.Description className="z-small" />
        <BaseToast.Action className="z-toast__action" />
      </BaseToast.Content>
      <BaseToast.Close className="z-toast__close" aria-label={t("ui.close")}>
        <CloseIcon size="sm" />
      </BaseToast.Close>
    </BaseToast.Root>
  ));
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
