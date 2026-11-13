import re

ROOT = "C:/Users/Abdi Guna Setiawan/Documents/Code/Zandor/"


def edit(p, pairs):
    s = open(ROOT + p, encoding="utf8").read()
    for a, b in pairs:
        assert a in s, (p, a[:70])
        s = s.replace(a, b, 1)
    open(ROOT + p, "w", encoding="utf8").write(s)


edit("packages/core/src/theme/appearance.ts", [
    ('        if (typeof v === "string" && validateTypeValue(tp, v) === null) (out.typography[tv] ??= {})[tp] = v;',
     '''        if (typeof v === "string" && validateTypeValue(tp, v) === null) {
          const cur = out.typography[tv] ?? {};
          cur[tp] = v;
          out.typography[tv] = cur;
        }'''),
])
# tes: set: (x) => void (v = x)
for p in ["packages/core/src/shortcuts-override.test.ts", "packages/core/src/theme/theme.test.ts", "packages/core/src/theme/vscode-theme.test.ts"]:
    s = open(ROOT + p, encoding="utf8").read()
    s2 = re.sub(r"set: \((\w+)(: string)?\) => void \((\w+) = \1\)", lambda m: "set: (%s%s) => {\n      %s = %s;\n    }" % (m.group(1), m.group(2) or "", m.group(3), m.group(1)), s)
    assert s != s2, p
    open(ROOT + p, "w", encoding="utf8").write(s2)

edit("packages/core/src/memory/store.ts", [
    ('    if (!entry || entry.status !== "pending") return', '    if (entry?.status !== "pending") return'),
])
edit("apps/desktop/src/shell/CommandCenter.tsx", [("[paletteOpen, list, query, translate, t, registry.comboOf]", "[paletteOpen, list, query, translate, t, registry]")])
edit("apps/desktop/src/shell/AppearancePage.tsx", [
    ('          <label key={k} className="ap__color">', '          <div key={k} className="ap__color">'),
])
s = open(ROOT + "apps/desktop/src/shell/AppearancePage.tsx", encoding="utf8").read()
# tutup </label> -> </div> hanya untuk blok ap__color
i = s.index('<div key={k} className="ap__color">')
j = s.index("</label>", i)
s = s[:j] + "</div>" + s[j + len("</label>"):]
open(ROOT + "apps/desktop/src/shell/AppearancePage.tsx", "w", encoding="utf8").write(s)

s = open(ROOT + "apps/desktop/src/shell/tools/InfoPanel.tsx", encoding="utf8").read()
s = s.replace("  // biome-ignore lint/correctness/useExhaustiveDependencies: sekali per sesi/project\n", "")
open(ROOT + "apps/desktop/src/shell/tools/InfoPanel.tsx", "w", encoding="utf8").write(s)

# CSS: pindahkan aturan `.msg .md` setelah `.md { ... }`
p = ROOT + "apps/desktop/src/styles.css"
s = open(p, encoding="utf8").read()
a = ".msg__text,\n.msg .md {\n  font-size: var(--z-chat-size);\n  font-weight: var(--z-chat-weight);\n  opacity: var(--z-chat-opacity);\n}\n"
assert a in s
s = s.replace(a, ".msg__text {\n  font-size: var(--z-chat-size);\n  font-weight: var(--z-chat-weight);\n  opacity: var(--z-chat-opacity);\n}\n", 1)
b = ".md {\n  overflow-wrap: anywhere;\n}\n"
assert b in s
s = s.replace(b, b + ".msg .md {\n  font-size: var(--z-chat-size);\n  font-weight: var(--z-chat-weight);\n  opacity: var(--z-chat-opacity);\n}\n", 1)
open(p, "w", encoding="utf8").write(s)
print("ok")
