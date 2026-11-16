import type { IconName } from "@agenteschat/ui";
import type { MouseEvent } from "react";
import { matchesNow, TOUCH } from "@/hooks/useMatchMedia";

export type MenuEntry =
  | { kind: "item"; id: string; label: string; icon?: IconName; danger?: boolean; disabled?: boolean; run: () => void }
  | { kind: "header"; label: string }
  | { kind: "separator" }
  | {
      kind: "reactions";
      label: string;
      emojis: readonly string[];
      pick: (emoji: string) => void;
      more?: { label: string; run: () => void };
    };

export const QUICK_REACTIONS = ["👍", "🎉", "👀", "✅", "❤️"] as const;

export function tidy(entries: MenuEntry[]): MenuEntry[] {
  const out: MenuEntry[] = [];
  for (const e of entries) {
    if (e.kind === "separator" && (out.length === 0 || out.at(-1)?.kind === "separator")) continue;
    out.push(e);
  }
  while (out.at(-1)?.kind === "separator") out.pop();
  return out;
}

export function keepsSystemMenu(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  if (target.closest("input, textarea, select, [contenteditable='true'], [contenteditable='']")) return true;
  return Boolean(target.closest("a[href], img, video, pre, code"));
}

export function selectedTextAt(target: EventTarget | null): string {
  if (!(target instanceof Node)) return "";
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || !selection.containsNode(target, true)) return "";
  return selection.toString().trim();
}

export function selectingIn(el: Node | null): boolean {
  const selection = typeof window.getSelection === "function" ? window.getSelection() : null;
  return !!el && !!selection && !selection.isCollapsed && el.contains(selection.anchorNode);
}

export interface OpenMenu {
  key: number;
  x: number;
  y: number;
  entries: MenuEntry[];
}

let show: ((menu: OpenMenu | null) => void) | null = null;
let seq = 0;

export function setMenuHost(host: ((menu: OpenMenu | null) => void) | null) {
  show = host;
}

let reporter: { label: string; run: (target: EventTarget, selected: string) => void } | null = null;
export function setMenuReporter(next: typeof reporter) {
  reporter = next;
}

export function openContextMenu(x: number, y: number, raw: MenuEntry[], target?: EventTarget | null) {
  const report = reporter;
  const selected = target ? selectedTextAt(target) : "";
  const entries = tidy(
    target && report
      ? [
          ...raw,
          { kind: "separator" },
          {
            kind: "item",
            id: "feedback.report",
            label: report.label,
            icon: "message",
            run: () => report.run(target, selected),
          },
        ]
      : raw,
  );
  if (!entries.length) return;
  show?.({ key: ++seq, x, y, entries });
}

export const contextMenusOn = () => show !== null;

export function contextMenuHandler(build: (e: MouseEvent) => MenuEntry[]) {
  return (e: MouseEvent) => {
    if (e.defaultPrevented || !contextMenusOn() || keepsSystemMenu(e.target)) return;
    if ((e.nativeEvent as PointerEvent).pointerType === "touch" || matchesNow(TOUCH)) return;
    const entries = build(e);
    if (!entries.length) return;
    e.preventDefault();
    openContextMenu(e.clientX, e.clientY, entries, e.target);
  };
}

export async function copyText(text: string, done: () => void) {
  try {
    await navigator.clipboard?.writeText(text);
    done();
  } catch {}
}
