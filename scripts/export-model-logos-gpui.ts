// Ekspor data logo model (packages/ui/src/model-logos.generated.ts) ke JSON untuk apps/desktop-gpui.
import { MODEL_LOGOS, MODEL_LOGOS_COLOR } from "../packages/ui/src/model-logos.generated";
await Bun.write("apps/desktop-gpui/assets/model-logos.json", JSON.stringify({ mono: MODEL_LOGOS, color: MODEL_LOGOS_COLOR }));
