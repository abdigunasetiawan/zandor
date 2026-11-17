// Ekspor katalog i18n TS ke JSON untuk apps/desktop-gpui (sumber kebenaran tetap packages/i18n).
import { catalogs } from "../packages/i18n/src/index";
for (const [loc, msgs] of Object.entries(catalogs)) {
  await Bun.write(`apps/desktop-gpui/assets/i18n/${loc}.json`, JSON.stringify(msgs, null, 1));
}
