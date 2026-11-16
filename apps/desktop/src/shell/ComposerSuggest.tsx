// Popup saran composer: perintah `/` dan mention `@berkas`. Keyboard ditangani textarea (↑↓, Enter/Tab, Esc) agar
// fokus tetap di input; popup hanya menampilkan daftar (role listbox) dan menerima klik.
import { useI18n } from "@zandor/i18n/react";
import type { ReactNode } from "react";
import { useEffect, useRef } from "react";

export type SuggestItem = {
  id: string;
  label: string;
  /** Teks redup sesudah label (mis. placeholder argumen). */
  hint?: string;
  description?: string;
  icon?: ReactNode;
  badge?: string;
  group?: string;
};

export function ComposerSuggest({
  id,
  title,
  items,
  active,
  status,
  onPick,
  onHover,
}: {
  id: string;
  title: string;
  items: SuggestItem[];
  active: number;
  /** Pesan status bila daftar kosong (memuat/kosong/galat). */
  status?: string | null;
  onPick: (index: number) => void;
  onHover: (index: number) => void;
}) {
  const { t } = useI18n();
  const listRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    listRef.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);
  return (
    <div className="csug z-popup">
      <div ref={listRef} id={id} className="csug__list" role="listbox" aria-label={title}>
        {items.map((item, i) => {
          const head = item.group && item.group !== items[i - 1]?.group;
          return (
            <div key={item.id} role="presentation">
              {head && (
                <div className="csug__group z-label" aria-hidden="true">
                  {item.group}
                </div>
              )}
              <div
                id={`${id}-${i}`}
                data-index={i}
                role="option"
                tabIndex={-1}
                aria-selected={i === active}
                className={item.description ? "z-item z-item--described csug__item" : "z-item csug__item"}
                data-highlighted={i === active ? "" : undefined}
                onMouseDown={(e) => {
                  e.preventDefault(); // fokus tetap di textarea
                  onPick(i);
                }}
                onMouseMove={() => i !== active && onHover(i)}
              >
                {item.icon && <span className="z-item__icon">{item.icon}</span>}
                <span className="z-item__text">
                  <span className="csug__line">
                    <span className="csug__label">{item.label}</span>
                    {item.hint && <span className="csug__hint">{item.hint}</span>}
                  </span>
                  {item.description && <span className="z-item__desc csug__desc">{item.description}</span>}
                </span>
                {item.badge && <span className="csug__badge z-small">{item.badge}</span>}
              </div>
            </div>
          );
        })}
      </div>
      {items.length === 0 && status && (
        <p className="csug__status z-small" role="status">
          {status}
        </p>
      )}
      <p className="csug__foot z-small">{t("composer.suggestHint")}</p>
    </div>
  );
}
