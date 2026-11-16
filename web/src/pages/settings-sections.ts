import type { AuthConfig, OrgSummary } from "@agenteschat/client";
import { showsAbout } from "@agenteschat/client/lib/about";
import { can } from "@agenteschat/client/lib/permissions";
import { personalSettingsShort } from "@agenteschat/client/lib/personal-view";
import { settingsEntry, teamSettings } from "@agenteschat/client/lib/settings-nav";
import { isPersonal } from "@agenteschat/client/lib/workspace-mode";
import type { IconName } from "@agenteschat/ui";

// Apart from the settings page so the shell (⌘K, the sidebar) and the settings routes' guards can read
// the sections without loading the page itself.

export type SettingsSection =
  | "account"
  | "notifications"
  | "general"
  | "connectors"
  | "skills"
  | "usage"
  | "about";

/**
 * Every settings section with who can open it on this server, in the order the list and each entry's tabs show them:
 * an entry, then the sections its page holds (`SETTINGS_PARTS`), then what goes under Advanced.
 */
export const SETTINGS_SECTIONS: {
  id: SettingsSection;
  icon: IconName;
  allowed: (org: OrgSummary, server: AuthConfig | undefined) => boolean;
}[] = [
  // Mine, not the workspace's: my profile here and what this device keeps (the nav shows my face for it).
  { id: "account", icon: "person", allowed: () => true },
  // What reaches my devices as a notification here (ADR 0071 §8).
  { id: "notifications", icon: "bell", allowed: () => true },
  // The workspace's sections are for who runs it: a member keeps only their own (above) and what they use themselves
  // (the people, where a member who may not invite suggests someone (ADR 0090), connectors, computers, AI accounts).
  { id: "general", icon: "settings", allowed: (o) => can(o, "configure") },
  // "IA e uso", for everyone (ADR 0063): the workspace's AI accounts with their meters, what each person does with
  // their own, and the agents whose details they see, in tokens and percentages. The workspace's API keys only for who
  // manages them, money and the workspace's report only for who has `view_usage`.
  { id: "usage", icon: "wallet", allowed: () => true },
  { id: "connectors", icon: "plug", allowed: () => true },
  // Every member keeps skills of their own there; the team's library is changed by who manages agents.
  { id: "skills", icon: "bulb", allowed: () => true },
  // AGPL-3.0's legal notices and the license's required attribution: for everyone on a self-hosted server, never
  // hidden there; none on agentes.chat Cloud (`showsAbout`).
  { id: "about", icon: "info", allowed: (_o, server) => showsAbout(server) },
];

export const isSettingsSection = (v: unknown): v is SettingsSection => SETTINGS_SECTIONS.some((s) => s.id === v);

/**
 * Sections that became another one's: Computers is Devices, AI accounts is part of AI and usage. Their addresses
 * still open, on the section that holds them now.
 */
export const MOVED_SETTINGS_SECTIONS: Readonly<Record<string, SettingsSection>> = {
  keys: "usage",
};

/** Where the workspace's right hand is set now: its own page. Its old settings address opens that. */
export const RIGHT_HAND_SECTION = "right_hand";

/**
 * The short list of this workspace's settings: a personal one's own (ADR 0159 §3, People on it once `others` are
 * members), a team's entries.
 */
export const settingsShort = (
  org: Pick<OrgSummary, "mode">,
  server: AuthConfig | undefined,
  others?: number,
): readonly string[] => (isPersonal(org) ? personalSettingsShort(others) : teamSettings(server?.cloud === true));

/** The nav entry that stands for the open section: the one whose page holds it, else the section itself. */
export function settingsNavEntry(
  current: SettingsSection,
  open: readonly SettingsSection[],
  short: readonly string[],
): SettingsSection {
  const entry = settingsEntry(current, new Set(open), short);
  return isSettingsSection(entry) ? entry : current;
}

/**
 * `?connected=github` (back from OAuth), `?review=1` (a custom MCP server to
 * classify), `?mail=connected|failed&account={id}` (back from Gmail/Outlook),
 * `?request={id}&do=approve|deny` (a request to join, from the admins' email),
 * `?delete=1` (the dialog that deletes the workspace, from Geral or Sua conta), `?create=1` (Cargos' Criar cargo, from ⌘K), `?install={link}` (Skills, back from connecting GitHub
 * in Instalar skill: the link to read again, with `agent` when an agent's card opened it) and `?agent={id}`
 * (Auditoria filtered to that agent).
 */
export const settingsSearch = (
  search: Record<string, unknown>,
): {
  connected?: string;
  outcome?: string;
  review?: string;
  delete?: 1;
  install?: string;
  skill?: string;
  agent?: string;
} => ({
  connected: typeof search.connected === "string" ? search.connected : undefined,
  outcome: typeof search.outcome === "string" ? search.outcome : undefined,
  review: search.review === undefined ? undefined : String(search.review),
  delete: search.delete === undefined ? undefined : 1,
  install: typeof search.install === "string" ? search.install : undefined,
  skill: typeof search.skill === "string" ? search.skill : undefined,
  agent: typeof search.agent === "string" ? search.agent : undefined,
});
