// Tombol "Kustomisasi" di bilah atas Panel 2 (di kiri tombol tutup): popover kontrol gaya chat yang sama dengan
// Pengaturan → Chat, plus tautan ke halaman Pengaturan.
import { useI18n } from "@zandor/i18n/react";
import { Button, CustomizeIcon, IconButton, Popover, SettingsIcon } from "@zandor/ui";
import { useState } from "react";
import { ChatStyleControls } from "./ChatStyleControls";
import { useServices } from "./services";

export function ChatCustomize() {
  const { t } = useI18n();
  const { nav } = useServices();
  const [open, setOpen] = useState(false);
  return (
    <Popover
      open={open}
      onOpenChange={setOpen}
      side="bottom"
      align="end"
      title={t("chatStyle.title")}
      className="cstyle-pop"
      trigger={<IconButton label={t("chatStyle.open")} size="sm" icon={<CustomizeIcon />} />}
    >
      <div className="cstyle-pop__body">
        <ChatStyleControls compact />
      </div>
      <div className="cstyle-pop__foot">
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            setOpen(false);
            nav.push("/settings/chat");
          }}
        >
          <SettingsIcon size="sm" />
          {t("chatStyle.allSettings")}
        </Button>
      </div>
    </Popover>
  );
}
