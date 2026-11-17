// Ekspor preset tema (sumber kebenaran packages/core/src/theme/presets.ts) ke JSON untuk apps/desktop-gpui.
import { PRESETS } from "../packages/core/src/theme/presets";
await Bun.write("apps/desktop-gpui/assets/presets.json", JSON.stringify(PRESETS, null, 1));
