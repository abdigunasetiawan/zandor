// Tombol "Kustomisasi" di bilah atas Panel 3 (di kiri tombol tutup): popover kontrol gaya untuk jenis dokumen aktif,
// sama dengan Pengaturan → Dokumen, plus tautan ke halaman Pengaturan.
import { useI18n } from "@zandor/i18n/react";
import { Button, CustomizeIcon, IconButton, Popover, SettingsIcon } from "@zandor/ui";
import { useState } from "react";
import { useServices } from "../services";
import { DocStyleControls, type DocStyleGroup } from "./DocStyleControls";

export function DocCustomize({ groups }: { groups: DocStyleGroup[] }) {
  const { t } = useI18n();
  const { nav } = useServices();
  const [open, setOpen] = useState(false);
  return (
    <Popover
      open={open}
      onOpenChange={setOpen}
      side="bottom"
      align="end"
      title={t("docStyle.title")}
      className="cstyle-pop"
      trigger={<IconButton label={t("docStyle.open")} size="sm" icon={<CustomizeIcon />} />}
    >
      <div className="cstyle-pop__body">
        <DocStyleControls groups={groups} compact />
      </div>
      <div className="cstyle-pop__foot">
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            setOpen(false);
            nav.push("/settings/documents");
          }}
        >
          <SettingsIcon size="sm" />
          {t("docStyle.allSettings")}
        </Button>
      </div>
    </Popover>
  );
}
