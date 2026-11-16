import { Tabs as BaseTabs } from "@base-ui/react/tabs";
import type { ReactNode } from "react";

export type TabItem = { value: string; label: string; content: ReactNode; disabled?: boolean; icon?: ReactNode };

export type TabsProps = {
  items: TabItem[];
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
  "aria-label"?: string;
  /** "vertical": daftar tab di sisi kiri, panel di kanan. */
  orientation?: "horizontal" | "vertical";
};

export function Tabs({ items, value, defaultValue, onValueChange, orientation = "horizontal", ...aria }: TabsProps) {
  return (
    <BaseTabs.Root
      className="z-tabs"
      orientation={orientation}
      value={value}
      defaultValue={defaultValue ?? items[0]?.value}
      onValueChange={(v) => onValueChange?.(String(v))}
    >
      <BaseTabs.List className="z-tabs__list" aria-label={aria["aria-label"]}>
        {items.map((it) => (
          <BaseTabs.Tab key={it.value} value={it.value} disabled={it.disabled} className="z-tabs__tab">
            {it.icon}
            {it.label}
          </BaseTabs.Tab>
        ))}
        <BaseTabs.Indicator className="z-tabs__indicator" />
      </BaseTabs.List>
      {items.map((it) => (
        <BaseTabs.Panel key={it.value} value={it.value} className="z-tabs__panel">
          {it.content}
        </BaseTabs.Panel>
      ))}
    </BaseTabs.Root>
  );
}
