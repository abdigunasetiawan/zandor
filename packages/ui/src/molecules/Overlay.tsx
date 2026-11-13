import { AlertDialog as BaseAlertDialog } from "@base-ui/react/alert-dialog";
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { Popover as BasePopover } from "@base-ui/react/popover";
import { useI18n } from "@zandor/i18n/react";
import { createContext, type ReactElement, type ReactNode, useCallback, useContext, useRef, useState } from "react";
import { Button } from "../atoms/Button";

export type PopoverProps = { trigger: ReactElement; title?: string; children: ReactNode };

export function Popover({ trigger, title, children }: PopoverProps) {
  return (
    <BasePopover.Root>
      <BasePopover.Trigger render={trigger} />
      <BasePopover.Portal>
        <BasePopover.Positioner sideOffset={6} className="z-popup-positioner">
          <BasePopover.Popup className="z-popup z-popover" data-shortcut-scope="popover">
            {title && <BasePopover.Title className="z-h4 z-popover__title">{title}</BasePopover.Title>}
            {children}
          </BasePopover.Popup>
        </BasePopover.Positioner>
      </BasePopover.Portal>
    </BasePopover.Root>
  );
}

export type DialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children?: ReactNode;
  /** Tombol aksi di bawah. Tombol "Tutup" ditambahkan otomatis bila tidak diisi. */
  footer?: ReactNode;
};

export function Dialog({ open, onOpenChange, title, description, children, footer }: DialogProps) {
  const { t } = useI18n();
  return (
    <BaseDialog.Root open={open} onOpenChange={onOpenChange}>
      <BaseDialog.Portal>
        <BaseDialog.Backdrop className="z-backdrop" />
        <BaseDialog.Popup className="z-dialog" data-shortcut-scope="modal">
          <BaseDialog.Title className="z-h3">{title}</BaseDialog.Title>
          {description && (
            <BaseDialog.Description className="z-p z-dialog__description">{description}</BaseDialog.Description>
          )}
          {children && <div className="z-dialog__body">{children}</div>}
          <div className="z-dialog__footer">
            {footer ?? <BaseDialog.Close render={<Button>{t("ui.close")}</Button>} />}
          </div>
        </BaseDialog.Popup>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
}

export type ConfirmOptions = {
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  /** "danger" → tombol konfirmasi berwarna bahaya. */
  tone?: "default" | "danger";
};

type ConfirmFn = (options: ConfirmOptions) => Promise<boolean>;

const ConfirmContext = createContext<ConfirmFn | null>(null);

/** Pengganti `window.confirm` (PRD §4.4): kembalikan Promise<boolean>; Esc/batal = false. */
export function useConfirm(): ConfirmFn {
  const fn = useContext(ConfirmContext);
  if (!fn) throw new Error("useConfirm harus dipakai di dalam <ConfirmProvider>");
  return fn;
}

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const { t } = useI18n();
  const [options, setOptions] = useState<ConfirmOptions | null>(null);
  const resolver = useRef<((v: boolean) => void) | null>(null);

  const confirm = useCallback<ConfirmFn>((opts) => {
    // Konfirmasi yang masih terbuka dibatalkan sebelum yang baru tampil.
    resolver.current?.(false);
    return new Promise<boolean>((resolve) => {
      resolver.current = resolve;
      setOptions(opts);
    });
  }, []);

  const settle = (value: boolean) => {
    resolver.current?.(value);
    resolver.current = null;
    setOptions(null);
  };

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      <BaseAlertDialog.Root open={options !== null} onOpenChange={(open) => !open && settle(false)}>
        <BaseAlertDialog.Portal>
          <BaseAlertDialog.Backdrop className="z-backdrop" />
          <BaseAlertDialog.Popup className="z-dialog" data-shortcut-scope="modal">
            <BaseAlertDialog.Title className="z-h3">{options?.title}</BaseAlertDialog.Title>
            {options?.description && (
              <BaseAlertDialog.Description className="z-p z-dialog__description">
                {options.description}
              </BaseAlertDialog.Description>
            )}
            <div className="z-dialog__footer">
              <Button onClick={() => settle(false)}>{options?.cancelLabel ?? t("ui.cancel")}</Button>
              <Button variant={options?.tone === "danger" ? "danger" : "solid"} onClick={() => settle(true)}>
                {options?.confirmLabel ?? t("ui.confirm")}
              </Button>
            </div>
          </BaseAlertDialog.Popup>
        </BaseAlertDialog.Portal>
      </BaseAlertDialog.Root>
    </ConfirmContext.Provider>
  );
}
