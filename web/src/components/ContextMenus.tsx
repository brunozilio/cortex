import { MenuAt, MenuHeading, MenuItem, MenuReactions, MenuSeparator } from "@agenteschat/ui";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type MenuEntry, type OpenMenu, setMenuHost } from "@/lib/context-menu";

/**
 * The right click: draws our menu when a screen opens one (a message, a channel). Links, empty space, typing and
 * selected text keep the browser's menu.
 */
export function ContextMenuHost() {
  const { t } = useTranslation();
  const [open, setOpen] = useState<OpenMenu | null>(null);
  const close = useCallback(() => setOpen(null), []);

  useEffect(() => {
    setMenuHost(setOpen);
    return () => setMenuHost(null);
  }, []);

  if (!open) return null;
  return (
    <MenuAt key={open.key} at={open} onClose={close} label={t("contextMenu.label")}>
      <MenuEntries entries={open.entries} onRun={close} />
    </MenuAt>
  );
}

/** Entries as our menu's items: a message's menu on touch, and the menus screens open on a right click. */
export function MenuEntries({ entries, onRun }: { entries: MenuEntry[]; onRun?: () => void }) {
  const run = (fn: () => void) => () => {
    onRun?.();
    fn();
  };
  return entries.map((e, i) => {
    const key = e.kind === "item" ? e.id : `${e.kind}-${i}`;
    if (e.kind === "separator") return <MenuSeparator key={key} />;
    if (e.kind === "header") return <MenuHeading key={key}>{e.label}</MenuHeading>;
    if (e.kind === "reactions") {
      const { pick, more } = e;
      return (
        <MenuReactions
          key={key}
          label={e.label}
          emojis={e.emojis}
          onPick={(emoji) => run(() => pick(emoji))()}
          more={more && { label: more.label, onSelect: run(more.run) }}
        />
      );
    }
    return (
      <MenuItem key={key} icon={e.icon} danger={e.danger} disabled={e.disabled} onSelect={run(e.run)}>
        {e.label}
      </MenuItem>
    );
  });
}
