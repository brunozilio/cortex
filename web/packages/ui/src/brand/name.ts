import { useSyncExternalStore } from "react";

const listeners = new Set<() => void>();

export const fallbackBrandName = (): string => {
  const host = typeof location === "undefined" ? "" : location.hostname;
  return host || "app";
};

let current: string | null = null;

export const brandName = (): string => current ?? fallbackBrandName();

export function setBrandName(name: string | null | undefined): void {
  const next = name?.trim() || null;
  if (next === current) return;
  current = next;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export const useBrandName = (): string => useSyncExternalStore(subscribe, brandName, brandName);

export function brandParts(name: string): { word: string; tld: string } {
  const dot = name.lastIndexOf(".");
  return dot > 0 && dot < name.length - 1 ? { word: name.slice(0, dot), tld: name.slice(dot) } : { word: name, tld: "" };
}
