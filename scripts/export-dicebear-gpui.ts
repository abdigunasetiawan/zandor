// Bundel DiceBear (paket yang sama dengan apps/desktop) menjadi satu IIFE untuk apps/desktop-gpui (QuickJS lokal).
const r = await Bun.build({ entrypoints: ["apps/desktop/src/gpui-export/dicebear-entry.ts"], format: "iife", minify: true, target: "browser" });
if (!r.success) throw new AggregateError(r.logs, "bundel DiceBear gagal");
await Bun.write("apps/desktop-gpui/assets/dicebear.js", await r.outputs[0].text());
console.log("ok", (await r.outputs[0].text()).length);
