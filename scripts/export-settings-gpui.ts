// Ekspor skema pengaturan (sumber kebenaran packages/core/src/settings.ts) ke JSON untuk apps/desktop-gpui.
import { schemaJson } from "../packages/core/src/settings";
await Bun.write("apps/desktop-gpui/assets/settings.schema.json", schemaJson());
