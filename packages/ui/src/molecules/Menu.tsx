import { ContextMenu as BaseContextMenu } from "@base-ui/react/context-menu";
import { Menu as BaseMenu } from "@base-ui/react/menu";
import type { CSSProperties, ReactElement, ReactNode } from "react";
import { Kbd } from "../atoms/Kbd";
import { cx } from "../cx";
import { CheckIcon } from "../icons";
import { OFFSET_TIGHT } from "../layout";

export type MenuEntry =
  | {
      type: "item";
      id: string;
      label: string;
      onSelect: () => void;
      shortcut?: string;
      /** Ikon di depan label (elemen ikon dari `@zandor/ui`, ukuran `sm`). */
      icon?: ReactNode;
      disabled?: boolean;
      danger?: boolean;
    }
  | {
      type: "checkbox";
      id: string;
      label: string;
      checked: boolean;
      onCheckedChange: (checked: boolean) => void;
      disabled?: boolean;
    }
  | { type: "separator"; id: string };

/** Bagian bersama Menu dan ContextMenu (nama part identik di Base UI). */
type MenuParts = {
  Item: typeof BaseMenu.Item;
  CheckboxItem: typeof BaseMenu.CheckboxItem;
  CheckboxItemIndicator: typeof BaseMenu.CheckboxItemIndicator;
  Separator: typeof BaseMenu.Separator;
};

function Entries({ parts, entries }: { parts: MenuParts; entries: MenuEntry[] }): ReactNode {
  const { Item, CheckboxItem, CheckboxItemIndicator, Separator } = parts;
  return entries.map((e) => {
    if (e.type === "separator") return <Separator key={e.id} className="z-separator" />;
    if (e.type === "checkbox") {
      return (
        <CheckboxItem
          key={e.id}
          checked={e.checked}
          onCheckedChange={e.onCheckedChange}
          disabled={e.disabled}
          className="z-item"
        >
          <span className="z-item__label">{e.label}</span>
          <CheckboxItemIndicator className="z-item__indicator">
            <CheckIcon size="sm" />
          </CheckboxItemIndicator>
        </CheckboxItem>
      );
    }
    return (
      <Item
        key={e.id}
        onClick={e.onSelect}
        disabled={e.disabled}
        className={cx("z-item", e.danger && "z-item--danger")}
      >
        {e.icon && (
          <span className="z-item__icon" aria-hidden="true">
            {e.icon}
          </span>
        )}
        <span className="z-item__label">{e.label}</span>
        {e.shortcut && <Kbd keys={e.shortcut} className="z-item__shortcut" />}
      </Item>
    );
  });
}

export type MenuProps = {
  /** Elemen pemicu tunggal (mis. <Button>). */
  trigger: ReactElement;
  entries: MenuEntry[];
};

export function Menu({ trigger, entries }: MenuProps) {
  return (
    <BaseMenu.Root>
      <BaseMenu.Trigger render={trigger} />
      <BaseMenu.Portal>
        <BaseMenu.Positioner sideOffset={OFFSET_TIGHT} align="start" className="z-popup-positioner">
          <BaseMenu.Popup className="z-popup" data-shortcut-scope="menu">
            <Entries parts={BaseMenu} entries={entries} />
          </BaseMenu.Popup>
        </BaseMenu.Positioner>
      </BaseMenu.Portal>
    </BaseMenu.Root>
  );
}

export type ContextMenuProps = {
  children: ReactNode;
  entries: MenuEntry[];
  className?: string;
  /** Atribut tambahan untuk elemen pemicu (gaya inline dan atribut data-*). */
  triggerProps?: { style?: CSSProperties; [key: `data-${string}`]: string | undefined };
};

/** Menu klik-kanan pada area `children`. */
export function ContextMenu({ children, entries, className, triggerProps }: ContextMenuProps) {
  return (
    <BaseContextMenu.Root>
      <BaseContextMenu.Trigger className={className} {...triggerProps}>
        {children}
      </BaseContextMenu.Trigger>
      <BaseContextMenu.Portal>
        <BaseContextMenu.Positioner className="z-popup-positioner">
          <BaseContextMenu.Popup className="z-popup" data-shortcut-scope="menu">
            <Entries parts={BaseContextMenu as unknown as MenuParts} entries={entries} />
          </BaseContextMenu.Popup>
        </BaseContextMenu.Positioner>
      </BaseContextMenu.Portal>
    </BaseContextMenu.Root>
  );
}
