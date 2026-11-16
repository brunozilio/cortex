import type { AccessTool, } from "@agenteschat/client";
import { keys, useCatalog, useConnections, useCustomServers } from "@agenteschat/client";
import {
  type AccessResource,
  type Level,
  type LevelState,
  saveLevels,
  toolsAt,
} from "@agenteschat/client/lib/access-levels";
import { type ConnectedItem, connectedItems, itemName } from "@agenteschat/client/lib/connected";
import type { agentGrammar } from "@agenteschat/client/lib/grammar";
import { addsConnectors, } from "@agenteschat/client/lib/permissions";
import { builtinToolText } from "@agenteschat/client/lib/scopes";
import { labelIsName } from "@agenteschat/client/lib/tool-labels";
import { partsOf, toolName } from "@agenteschat/client/lib/tool-review";
import {
  type AgentAppearance,
  Button,
  Chip,
  CountChip,
  cx,
  EmptyState,
  Icon,
  type IconName,
  InfoHint,
  Segmented,
} from "@agenteschat/ui";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import type { TFunction } from "i18next";
import { type ReactNode, useMemo, } from "react";
import { useTranslation } from "react-i18next";
import { useApi } from "@/api/context";
import { ConnectorIcon } from "@/components/ConnectorCatalog";
import { useOrg } from "@/components/org-context";
import { softBreaks } from "@/lib/soft-breaks";

type TT = (key: string, options: Record<string, string>) => string;

const AREA_ICON: Record<string, IconName> = {
  context: "search",
  drive: "drive",
  issues: "issues",
  calendar: "calendar",
  wiki: "book",
  skills: "bolt",
  schedule: "routines",
  sandbox: "computer",
  browser: "globe",
  setup: "edit",
  desktop: "pointer",
};

/** The connected services by connection id (their names and brand icons). */
export function useConnectedById() {
  const { org } = useOrg();
  const catalog = useCatalog();
  const connections = useConnections(org.id);
  const servers = useCustomServers(org.id);
  return useMemo(
    () =>
      new Map(
        connectedItems(connections.data ?? [], servers.data ?? [], catalog.data?.connectors ?? []).map((i) => [
          i.id,
          i,
        ]),
      ),
    [connections.data, servers.data, catalog.data],
  );
}

/** A resource's name in the app's language: the service's name, or the built-in area's. */
export function resourceName(t: TFunction, r: AccessResource, items: Map<string, ConnectedItem>) {
  if (r.connectionId) {
    const item = items.get(r.connectionId);
    return item ? itemName(t, item) : r.label;
  }
  return (t as unknown as TT)(`access.area.${r.area}`, { defaultValue: r.label });
}

export function ResourceIcon({
  resource,
  items,
  name,
}: {
  resource: Pick<AccessResource, "connectionId" | "area">;
  items: Map<string, ConnectedItem>;
  name: string;
}) {
  if (resource.connectionId) {
    const item = items.get(resource.connectionId);
    return <ConnectorIcon def={item?.def ?? null} image={item?.image} label={name} />;
  }
  return (
    <span className="connector-icon connector-icon--mono" aria-hidden="true">
      <Icon name={AREA_ICON[resource.area ?? ""] ?? "settings"} />
    </span>
  );
}

/**
 * A tool in words, "Ler arquivo" for `github_get_file_contents` (`toolName`: the catalog, else its verb and nouns); a
 * custom tool's label when someone named it; built-ins translated.
 */
export function toolLabel(t: TFunction, r: AccessResource, tool: AccessTool) {
  if (r.kind === "builtin") return builtinToolText(t, tool).label;
  if (r.custom && !labelIsName(tool.label, tool.name)) return tool.label;
  const prefix = r.connector && !r.custom ? `${r.connector}_` : "";
  // A numbered connection's tools (`linear_2_create_issue`) lose the number too.
  const own = prefix && tool.name.startsWith(prefix) ? tool.name.slice(prefix.length).replace(/^\d+_/, "") : tool.name;
  return toolName(t, partsOf(own), own);
}

/**
 * What a level means for a resource: a sentence for built-in areas (in the agent's own words, `agentGrammar`), the
 * tools for connectors. Nothing for "Sem acesso": the control says it.
 */
export function levelText(t: TFunction, r: AccessResource, level: LevelState, g: ReturnType<typeof agentGrammar>) {
  if (level === "none") return null;
  if (level === "custom") return t("access.customText");
  if (r.kind === "builtin") {
    const text = (t as unknown as TT)(`access.areaText.${r.area}.${level}`, {
      name: g.name,
      context: g.context ?? "",
      defaultValue: "",
    });
    if (text) return text;
  }
  const names = toolsAt(r, level)
    .filter((x) => !x.sensitive)
    .map((x) => toolLabel(t, r, x));
  if (names.length === 0) return t("access.noTools");
  return names.length > 6
    ? t("access.toolsMore", { tools: names.slice(0, 6).join(" · "), count: names.length - 6 })
    : names.join(" · ");
}

/** Saves levels for one agent (reads its grants fresh, one PUT) and refreshes the shared cache. */
export function useSetLevels() {
  const api = useApi();
  const { org } = useOrg();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ agentId, changes }: { agentId: string; changes: Record<string, Level> }) =>
      saveLevels(api, org.id, agentId, changes),
    onSuccess: (saved, { agentId }) => {
      qc.setQueryData(keys.agentAccess(org.id, agentId), saved);
      void qc.invalidateQueries({ queryKey: keys.scopes(org.id, agentId) });
    },
  });
}

/**
 * One resource and its level: icon, name and the choice (Sem acesso / Ler / Ler e escrever) on one line, what the
 * level gives behind an ⓘ beside the name, and under them one line of facts (what asks for approval, the row's own
 * settings). Without `onChange` it only shows the level, as a chip, or as the faded control (`locked`) in a list whose
 * other rows can be changed. The agent page and "Criar agente".
 */
export function LevelRow({
  icon,
  name,
  meta,
  hint,
  asks = [],
  levels,
  value,
  onChange,
  tag,
  side,
  extra,
  below,
  belowHidden,
  changed,
  locked,
}: {
  icon: ReactNode;
  name: string;
  /**
   * A short fact under the name, inside the row (the account, a server's host). A list passes it on every row, `null`
   * where there is none, so its rows share one height and one axis.
   */
  meta?: string | null;
  /** What the chosen level gives, behind an ⓘ beside the name. */
  hint?: ReactNode;
  asks?: string[];
  levels: Level[];
  value: LevelState;
  onChange?: (level: Level) => void;
  /** Next to the name ("Alterado" on the agent page). */
  tag?: ReactNode;
  /** A quiet action before the level, on the name's line ("Regras"). */
  side?: ReactNode;
  /** The row's quiet facts and small settings, after what asks (what it was before, who gives the access, a switch). */
  extra?: ReactNode;
  /** A full-width part under the row (GitHub: which repositories). */
  below?: ReactNode;
  /** `below` stays in the page, out of sight (a closed panel that keeps what was typed in it). */
  belowHidden?: boolean;
  /** Marked as changed and not saved yet. */
  changed?: boolean;
  /** Without `onChange`: the level control itself, faded, so the row keeps its neighbours' shape. */
  locked?: boolean;
}) {
  const { t } = useTranslation();
  const options = [
    ...levels.map((l) => ({ value: l as LevelState, label: t(`access.level.${l}`) })),
    ...(value === "custom" ? [{ value: "custom" as LevelState, label: t("access.level.custom") }] : []),
  ];
  return (
    <li
      className={cx("level-row ac-mediarow", meta !== undefined && "level-row--meta")}
      data-changed={changed || undefined}
    >
      <span className="ac-mediarow__face">{icon}</span>
      <span className="ac-mediarow__head">
        <span className="level-row__name">
          <span className="ac-hintline">
            {/* A long mailbox address breaks at its own joints before it breaks mid-word. */}
            <span>{softBreaks(name, "@.")}</span>
            {hint && <InfoHint name={t("common.about", { label: name })}>{hint}</InfoHint>}
          </span>
          {tag}
        </span>
        {meta && <span className="ac-mediarow__meta">{meta}</span>}
      </span>
      {side && <span className="level-row__side">{side}</span>}
      <span className="ac-mediarow__end ac-mediarow__end--wrap">
        {onChange || locked ? (
          <Segmented
            size="sm"
            className="level-row__choice"
            label={t("access.levelOf", { name })}
            value={value}
            disabled={!onChange}
            onValueChange={(l) => l !== "custom" && onChange?.(l)}
            options={options}
          />
        ) : (
          <Chip icon={value === "none" ? "dash" : value === "custom" ? "settings" : "check"}>
            {t(`access.level.${value}`)}
          </Chip>
        )}
      </span>
      <span
        className="ac-mediarow__body level-row__text"
        hidden={asks.length === 0 && !extra && (!below || belowHidden)}
      >
        {(asks.length > 0 || extra) && (
          <span className="level-row__facts">
            {asks.length > 0 && (
              <span className="level-row__asks ac-iconline">
                <Icon name="lock" size={14} />
                <span>
                  {t("access.asksList", { tools: asks.slice(0, 3).join(", ") })}
                  {asks.length > 3 && ` ${t("access.asksMore", { count: asks.length - 3 })}`}
                </span>
              </span>
            )}
            {extra}
          </span>
        )}
        {below && (
          <div className="level-row__below" hidden={belowHidden}>
            {below}
          </div>
        )}
      </span>
    </li>
  );
}

/** "Regras" on a row's name line: opens the row's rules under it, with how many there are. */
export function RulesToggle({
  label,
  count,
  open,
  controls,
  onToggle,
}: {
  label: string;
  count: number;
  open: boolean;
  controls: string;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  return (
    <>
      <Button variant="link" aria-label={label} aria-expanded={open} aria-controls={controls} onClick={onToggle}>
        {t("mail.agent.rules")}
      </Button>
      {count > 0 && <CountChip size="sm">{count}</CountChip>}
    </>
  );
}

/** No connector in the workspace yet, on an agent's access and on the matrix: the way to it, or who has it. */
export function NoConnectors({ look }: { look: AgentAppearance | true }) {
  const { t } = useTranslation();
  const { org } = useOrg();
  const admin = addsConnectors(org);
  return (
    <EmptyState
      size="panel"
      agent={look}
      title={t(admin ? "access.noConnectorsTitle" : "access.noConnectorsViewerTitle")}
      actions={
        admin && (
          <Button asChild variant="primary" icon="plus">
            <Link to="/org/$orgId/settings/$section" params={{ orgId: org.id, section: "connectors" }}>
              {t("connectors.add")}
            </Link>
          </Button>
        )
      }
    >
      {t(admin ? "access.noConnectors" : "access.noConnectorsViewer")}
    </EmptyState>
  );
}
