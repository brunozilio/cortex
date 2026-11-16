import type { Agent, Issue, } from "@agenteschat/client";
import {
  agentStateIn,
  useAgentDetail,
  useIssues,
  useUsage,
} from "@agenteschat/client";
import { seesDetails } from "@agenteschat/client/lib/audience";
import { ErrorBoundary } from "@agenteschat/client/lib/error-boundary";
import { formatPercent, formatUsd, } from "@agenteschat/client/lib/format";
import { isAssignee, STATUS_ICON } from "@agenteschat/client/lib/issues";
import { can } from "@agenteschat/client/lib/permissions";
import { usageShown } from "@agenteschat/client/lib/plan-usage";
import { agentFaceWord } from "@agenteschat/client/lib/presence";
import { isRightHand } from "@agenteschat/client/lib/right-hand";
import { Button, GroupHeading, Icon, InfoHint, PresenceBadge, } from "@agenteschat/ui";
import { Link, } from "@tanstack/react-router";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { AgentAvatar } from "@/components/AgentAvatar";
import { usd } from "@/components/AgentFields";
import { ModelPickerButton } from "@/components/ai/ModelPicker";
import { OpensAgent } from "@/components/OpensAgent";
import { useOrg } from "@/components/org-context";
import { StatusLine } from "@/components/Presence";
import { RightHandMineRow, useRightHandMineHint } from "@/components/RightHandSettings";
import { SidePanel } from "@/components/SidePanel";
import { AgentCover, colorOf } from "@/components/Sky";
import { useAgentThinks } from "@/hooks/useAgentThinks";
import { useDirectory } from "@/hooks/useDirectory";
import { useEscClose } from "@/hooks/useEscClose";
import { agentLine, useAgentPresence } from "@/hooks/usePresence";
import { useStreams } from "@/realtime/context";
import "./agent-sidebar.css";

/** While the sidebar shows, whether its screen started or stopped is asked again this often. */
const ROWS = 3;

/**
 * The right side of a DM with an agent, under the header where a channel's members sit: who it is, its computer, its
 * routines, how it thinks and its recent tasks, in one scroll. What the viewer may not see isn't drawn: without the
 * details (`seesDetails`), only its head. `onClose`: it floats over the chat, as the app's panels do.
 */
export function AgentSidebar({ agent, onClose }: { agent: Agent; onClose?: () => void }) {
  const { t } = useTranslation();
  const mineHint = useRightHandMineHint(agent);
  const body = (
    <ErrorBoundary resetKey={agent} fallback={() => null}>
      <AgentHead agent={agent} />
      {isRightHand(agent) ? (
        // No computer of its own, and each person's DM thinks with their own plan (ADR 0137 §11).
        <Section title={t("agentSide.thinks")} hint={mineHint}>
          <RightHandMineRow agent={agent} />
        </Section>
      ) : (
        seesDetails(agent) && (
          <>
            <ThinksSection agent={agent} />
            <TasksSection agent={agent} />
          </>
        )
      )}
    </ErrorBoundary>
  );
  if (!onClose) {
    return (
      <aside className="members-side dm-side" aria-label={t("agentSide.label")}>
        <div className="dm-side__scroll">{body}</div>
      </aside>
    );
  }
  return <FloatingSide onClose={onClose}>{body}</FloatingSide>;
}

function FloatingSide({ onClose, children }: { onClose: () => void; children: ReactNode }) {
  const { t } = useTranslation();
  useEscClose(onClose);
  return (
    <SidePanel className="dm-side dm-side--float" label={t("agentSide.label")} onClose={onClose}>
      <Button
        size="sm"
        iconOnly
        icon="close"
        className="agp-glass dm-side__close"
        aria-label={t("common.close")}
        title={t("common.close")}
        onClick={onClose}
      />
      <div className="dm-side__scroll">{children}</div>
    </SidePanel>
  );
}

function Section({
  title,
  count,
  action,
  hint,
  children,
}: {
  title: string;
  count?: number;
  action?: ReactNode;
  /** What the section is for, behind an ⓘ after its title, on the title's capitals. */
  hint?: { name: string; text: ReactNode };
  children: ReactNode;
}) {
  return (
    <section className="dm-side__sec" aria-label={title}>
      <GroupHeading
        level={3}
        title={
          hint ? (
            <span className="ac-hintline">
              <span>{title}</span>
              <InfoHint name={hint.name}>{hint.text}</InfoHint>
            </span>
          ) : (
            title
          )
        }
        count={count}
        aside={action}
      />
      {children}
    </section>
  );
}

function AgentHead({ agent }: { agent: Agent }) {
  const { t, i18n } = useTranslation();
  const shown = useAgentPresence(agent.id);
  const busy = useStreams((s) => agentStateIn(s, agent.id) !== "idle");
  const line = agentLine(t, shown?.line ?? null, i18n.language);
  const word = busy ? t("team.workingNow") : shown ? agentFaceWord(t, shown.presence) : null;
  // The right hand's one role name, as the sidebar and its DM's header say it.
  const role = isRightHand(agent) ? t("rightHand.labelCapitalized") : agent.role || `@${agent.handle}`;
  return (
    <header className="dm-side__head">
      <div className="agp-cover dm-side__cover">
        <AgentCover color={colorOf(agent)} busy={busy} />
      </div>
      <span className="dm-side__face">
        <OpensAgent agentId={agent.id}>
          <AgentAvatar agentId={agent.id} name={agent.name} size="lg" />
        </OpensAgent>
      </span>
      <h2 className="dm-side__name">{agent.name}</h2>
      <p className="dm-side__role">{role}</p>
      {line ? (
        <StatusLine {...line} className="dm-side__status" />
      ) : (
        word && (
          <p className="dm-side__status">
            {/* The face's own badge, as beside its face everywhere: at work its dots, offline its outline. */}
            <PresenceBadge presence={busy ? "working" : (shown?.presence ?? "offline")} ring={false} />
            {word}
          </p>
        )
      )}
    </header>
  );
}

function ThinksSection({ agent }: { agent: Agent }) {
  const { t, i18n } = useTranslation();
  const { org } = useOrg();
  const dir = useDirectory(org.id);
  const thinks = useAgentThinks(agent);
  const detail = useAgentDetail(org.id, agent.id);
  const admin = can(org, "manage_agents");
  const owner = agent.ownerUserId ? dir.memberById.get(agent.ownerUserId) : undefined;
  const usage = useUsage(org.id);
  const used = detail.data?.budgetUsedPercent ?? 0;
  const spent = usage.data?.find((u) => u.agentId === agent.id)?.costUsd;
  const limit = detail.data?.monthlyBudgetUsd ?? detail.data?.defaultBudgetUsd;
  return (
    <Section title={t("agentSide.thinks")}>
      <dl className="dm-side__facts">
        {thinks?.text && (
          <div>
            <dt>{t("agentSide.thinksWith")}</dt>
            <dd>
              <ModelPickerButton agent={agent} size="row" />
              {agent.brain?.speed && ` · ${t("agentPage.work.brain.speeds.fast")}`}
            </dd>
          </div>
        )}
        <div>
          <dt>{t("agentSide.owner")}</dt>
          <dd>{owner?.name ?? t("newAgent.noOwnerOption")}</dd>
        </div>
        {detail.data && usageShown(agent.brain, used).budget && (
          <div>
            <dt>{t("agentSide.usage")}</dt>
            <dd className="ac-num">
              {admin && spent !== undefined && limit !== undefined
                ? t("agentSide.spentOf", {
                    spent: formatUsd(i18n.language, spent),
                    limit: usd(t, i18n.language, limit),
                  })
                : formatPercent(i18n.language, used)}
            </dd>
          </div>
        )}
      </dl>
    </Section>
  );
}

function TasksSection({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const { org } = useOrg();
  const issues = useIssues(org.id);
  const recent = (issues.data ?? [])
    .filter((i) => isAssignee(i, "agent", agent.id))
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    .slice(0, ROWS);
  if (recent.length === 0) return null;
  return (
    <Section
      title={t("agentSide.tasks")}
      action={
        <Link to="/org/$orgId/issues" params={{ orgId: org.id }} className="ac-link dm-side__all">
          {t("agentSide.seeAll")}
        </Link>
      }
    >
      <ul className="dm-side__rows">
        {recent.map((i) => (
          <TaskLine key={i.id} issue={i} />
        ))}
      </ul>
    </Section>
  );
}

function TaskLine({ issue }: { issue: Issue }) {
  const { t } = useTranslation();
  const { org } = useOrg();
  return (
    <li>
      <Link
        to="/org/$orgId/issues/$number"
        params={{ orgId: org.id, number: String(issue.number) }}
        className="dm-side__row dm-side__row--link"
      >
        <span className="dm-side__stage" data-status={issue.status}>
          <Icon name={STATUS_ICON[issue.status]} size={16} />
        </span>
        <span className="dm-side__text">
          <b>{issue.title}</b>
          <span>{t(`issues.status.${issue.status}`)}</span>
        </span>
      </Link>
    </li>
  );
}
