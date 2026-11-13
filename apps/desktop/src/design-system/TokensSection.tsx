// Bagian "Tokens": semua nilai dibaca dari CSS (getComputedStyle), jadi halaman ini tidak punya nilai sendiri.
import { useEffect, useState } from "react";

export const COLOR_TOKENS = [
  "bg",
  "surface",
  "raised",
  "hover",
  "active",
  "border",
  "border-strong",
  "fg",
  "fg-muted",
  "fg-subtle",
  "accent",
  "on-accent",
  "focus",
  "danger",
  "success",
  "warning",
] as const;

export const TYPE_VARIABLES = ["h1", "h2", "h3", "h4", "p", "small", "link", "button", "label", "code"] as const;
export const SPACE_TOKENS = [1, 2, 3, 4, 5, 6, 7] as const;
export const RADIUS_TOKENS = ["sm", "md", "lg", "full"] as const;
export const SHADOW_TOKENS = [1, 2, 3] as const;

/** Naik setiap kali tema/skala berubah, dari mana pun asalnya (kontrol halaman, atribut, atau tema sistem OS). */
function useThemeVersion(): number {
  const [version, setVersion] = useState(0);
  useEffect(() => {
    const bump = () => setVersion((v) => v + 1);
    const observer = new MutationObserver(bump);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "style"] });
    const media = window.matchMedia("(prefers-color-scheme: light)");
    media.addEventListener("change", bump);
    return () => {
      observer.disconnect();
      media.removeEventListener("change", bump);
    };
  }, []);
  return version;
}

function useTokenValues(names: string[]): Record<string, string> {
  const version = useThemeVersion();
  const [values, setValues] = useState<Record<string, string>>({});
  // biome-ignore lint/correctness/useExhaustiveDependencies: dibaca ulang setiap tema/skala berubah (version)
  useEffect(() => {
    const cs = getComputedStyle(document.documentElement);
    setValues(Object.fromEntries(names.map((n) => [n, cs.getPropertyValue(n).trim()])));
  }, [version]);
  return values;
}

export function TokensSection() {
  const names = [
    ...COLOR_TOKENS.map((n) => `--z-color-${n}`),
    ...SPACE_TOKENS.map((n) => `--z-space-${n}`),
    ...TYPE_VARIABLES.flatMap((v) => [`--z-type-${v}-size`, `--z-type-${v}-weight`]),
  ];
  const v = useTokenValues(names);

  return (
    <div className="ds-stack ds-gap-lg">
      <div>
        <h3 className="z-h4">Warna</h3>
        <p className="z-small">Monokrom tanpa warna primary. Merah/hijau/kuning hanya untuk status.</p>
        <div className="ds-swatches" data-testid="swatches">
          {COLOR_TOKENS.map((n) => (
            <figure key={n} className="ds-swatch">
              <div className="ds-swatch__chip" style={{ background: `var(--z-color-${n})` }} />
              <figcaption>
                <code className="z-code">--z-color-{n}</code>
                <span className="z-small">{v[`--z-color-${n}`]}</span>
              </figcaption>
            </figure>
          ))}
        </div>
      </div>

      <div>
        <h3 className="z-h4">Tipografi per variabel</h3>
        <p className="z-small">Setiap variabel punya ukuran, ketebalan, tinggi baris, dan jarak huruf sendiri.</p>
        <div className="ds-type">
          {TYPE_VARIABLES.map((n) => (
            <div key={n} className="ds-type__row">
              <code className="z-code ds-type__name">
                {n} · {v[`--z-type-${n}-size`]} / {v[`--z-type-${n}-weight`]}
              </code>
              <span className={n === "link" ? "z-link" : `z-${n}`}>Zandor memakai banyak AI sekaligus</span>
            </div>
          ))}
        </div>
      </div>

      <div>
        <h3 className="z-h4">Ruang</h3>
        <div className="ds-stack">
          {SPACE_TOKENS.map((n) => (
            <div key={n} className="ds-space">
              <code className="z-code ds-space__name">--z-space-{n}</code>
              <div className="ds-space__bar" style={{ width: `var(--z-space-${n})` }} />
              <span className="z-small">{v[`--z-space-${n}`]}</span>
            </div>
          ))}
        </div>
      </div>

      <div>
        <h3 className="z-h4">Radius dan bayangan</h3>
        <div className="ds-row">
          {RADIUS_TOKENS.map((n) => (
            <div key={n} className="ds-box" style={{ borderRadius: `var(--z-radius-${n})` }}>
              <code className="z-code">radius-{n}</code>
            </div>
          ))}
          {SHADOW_TOKENS.map((n) => (
            <div key={n} className="ds-box" style={{ boxShadow: `var(--z-shadow-${n})` }}>
              <code className="z-code">shadow-{n}</code>
            </div>
          ))}
        </div>
      </div>

      <div>
        <h3 className="z-h4">Font</h3>
        <p style={{ fontFamily: "var(--z-font-ui)" }}>UI — Plus Jakarta Sans Variable (OFL)</p>
        <p className="z-code">Kode — JetBrains Mono Variable (OFL): const x = 42;</p>
        <p className="z-small">
          Font terminal memakai font kode secara bawaan dan bisa diatur terpisah (--z-font-terminal).
        </p>
      </div>
    </div>
  );
}
