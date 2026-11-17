// Ekspor ikon Lucide (yang dipakai packages/ui/src/icons.tsx) ke SVG untuk apps/desktop-gpui.
import { renderToStaticMarkup } from "react-dom/server";
import * as Icons from "../src/icons";
const out: string[] = [];
for (const [name, C] of Object.entries(Icons)) {
  if (!name.endsWith("Icon") || typeof C !== "function") continue;
  const svg = renderToStaticMarkup(<C />).replace(/ class="[^"]*"/, "").replace(/ aria-hidden="true"| focusable="false"/g, "");
  const file = name.replace(/Icon$/, "").replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
  await Bun.write(`../../apps/desktop-gpui/assets/icons/${file}.svg`, svg);
  out.push(file);
}
console.log(out.length, out.join(" "));
