// Salin aset xterm.js ke ui/vendor (spike tanpa bundler).
import { copyFileSync, mkdirSync } from "node:fs";
const out = new URL("../ui/vendor/", import.meta.url);
mkdirSync(out, { recursive: true });
for (const [from, to] of [
  ["@xterm/xterm/lib/xterm.js", "xterm.js"],
  ["@xterm/xterm/css/xterm.css", "xterm.css"],
  ["@xterm/addon-fit/lib/addon-fit.js", "addon-fit.js"],
  ["@xterm/addon-unicode11/lib/addon-unicode11.js", "addon-unicode11.js"],
]) copyFileSync(new URL(`../node_modules/${from}`, import.meta.url), new URL(to, out));
console.log("vendor ok");
