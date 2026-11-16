import { addDm } from "@agenteschat/client";
import { greetingSuggestions } from "@agenteschat/client/lib/chat";
import { isGone } from "@agenteschat/client/lib/errors";
import { agentGrammar } from "@agenteschat/client/lib/grammar";
import { isRightHand, rightHandSeenKey } from "@agenteschat/client/lib/right-hand";
import { Button, EmptyState } from "@agenteschat/ui";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { type ReactNode, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useApi } from "@/api/context";
import { AgentAvatar } from "@/components/AgentAvatar";
import { AgentSetupIntro, useSetupSteps } from "@/components/AgentSetup";
import { useAgentPanelShown, useOpenAgentPanel } from "@/components/agent-panel-context";
import { ModelPickerButton } from "@/components/ai/ModelPicker";
import { ErrorState } from "@/components/Loading";
import { NavToggle } from "@/components/NavDrawer";
import { useOrg } from "@/components/org-context";
import { RightHandIntro } from "@/components/RightHandIntro";
import { HeaderSearch } from "@/components/search/HeaderSearch";
import { AgentTasksPill } from "@/components/tasks/AgentTasksPanel";
import { useDirectory } from "@/hooks/useDirectory";
import { rightHandSeen } from "@/lib/storage";
import { ChannelPage, ChannelSkeleton, type DmSideControl } from "./ChannelPage";

/** A direct message with an agent: the channel layout with its own header and empty state. */
export function DmPage({ target, focusMessageId }: { target: { agentId: string }; focusMessageId?: string }) {
  const { t } = useTranslation();
  const { org, me } = useOrg();
  const api = useApi();
  const qc = useQueryClient();
  const dir = useDirectory(org.id);
  const id = target.agentId;
  const dm = useQuery({
    queryKey: ["org", org.id, "dm", "agent", id],
    queryFn: async () => {
      const res = await api.dms.open(org.id, target);
      // A DM opened for the first time is in the DM list from its own answer: its search holds to it.
      if (res.dm) addDm(qc, org.id, res.dm);
      return res;
    },
    staleTime: Number.POSITIVE_INFINITY,
  });

  const agent = dir.agentById.get(target.agentId);
  const name = agent?.name ?? "";
  const rightHand = isRightHand(agent);
  // A new agent's setup checklist (ADR 0155 §2) for whoever sets it up: its greeting and the checklist on top.
  const setup = useSetupSteps(agent).length > 0;
  // Its DM is open, from its row or any other way in: its row's "Novo" goes.
  useEffect(() => {
    if (rightHand) rightHandSeen.mark(rightHandSeenKey(me.user.id, org.id));
  }, [rightHand, me.user.id, org.id]);

  if (dm.isError) {
    return (
      <main className="shell__main">
        <div className="page page--mid">
          {isGone(dm.error) ? (
            <EmptyState icon="agent">{t("agent.notFound")}</EmptyState>
          ) : (
            <ErrorState error={dm.error} onRetry={() => void dm.refetch()} />
          )}
        </div>
      </main>
    );
  }
  if (!dm.data || dir.loading) return <ChannelSkeleton dm="agent" />;

  const owner = agent?.ownerUserId ? dir.memberById.get(agent.ownerUserId) : undefined;
  const role = agent ? agent.role || `@${agent.handle}` : "";
  // The right hand (ADR 0137) says what it is and whose permissions it acts with, never an owner, nor a model: each
  // person's DM with it thinks with their own AI, shown with its maker's logo in the side panel's Como pensa.
  const agentLine = agent
    ? rightHand
      ? `${t("rightHand.label")} · ${t("rightHand.actsAsYou")}`
      : owner
        ? t("dm.subtitle", { role, owner: owner.name })
        : t("dm.subtitleNoOwner", { role })
    : undefined;
  const header = ({
    onSearch,
    side,
    searching,
  }: {
    onSearch: () => void;
    side: DmSideControl | null;
    searching: boolean;
  }) => (
    <header className="channel__header">
      <NavToggle />
      {/* Who it is with and the header's glyphs: the glyphs go under a name that can't keep its line beside them. */}
      <div className="ac-actline channel__line">
        <DmWho agentId={agent?.id} label={t("profile.of", { name })} tip={agentLine}>
          <div className="dm-who__text">
            {agent && <AgentAvatar agentId={agent.id} name={agent.name} />}
            <h1 className={agent ? "channel__title ac-captop" : "channel__title"}>
              <span className="channel__name">{name}</span>
            </h1>
            {agent && (
              <span className="channel__topic channel__topic--line" data-right-hand={rightHand || undefined}>
                {agentLine}
              </span>
            )}
          </div>
        </DmWho>
        <div className="channel__acts">
          {/* Its tasks (ADR 0128), while it has any: they open in the side panel. */}
          {agent && <AgentTasksPill />}
          {agent && !rightHand && <ModelPickerButton agent={agent} className="channel__brain" />}
          {/* The same field as a channel's, held to this DM. */}
          <HeaderSearch channelId={dm.data.channelId} onOpen={onSearch} searching={searching} dm />
          {side && (
            <Button
              size="sm"
              variant="ghost"
              iconOnly
              icon="sidebar"
              className="channel__icon dm-side-toggle"
              aria-label={t("agentSide.label")}
              aria-pressed={side.open}
              title={t(side.open ? "agentSide.hide" : "agentSide.show")}
              onClick={side.toggle}
            />
          )}
        </div>
      </div>
    </header>
  );
  return (
    <ChannelPage
      channelId={dm.data.channelId}
      focusMessageId={focusMessageId}
      fallback={{
        id: dm.data.channelId,
        name,
        topic: "",
        visibility: "private",
        isMember: true,
        createdAt: "",
      }}
      chrome={{
        header,
        composerTarget: name,
        ...(rightHand && agent
          ? {
              composerLabel: `${t("channel.composerDm", { name })} · ${t("rightHand.composerHint")}`,
              intro: <RightHandIntro agent={agent} channelId={dm.data.channelId} />,
            }
          : setup && agent
            ? { intro: <AgentSetupIntro agent={agent} /> }
            : {}),
        dmAgentId: agent?.id,
        emptyText: agent ? t("dm.emptyAgent", agentGrammar(agent.name)) : undefined,
        emptyTitle: agent ? t("chat.sayHiAgent", agentGrammar(agent.name)) : undefined,
        suggestions: agent ? greetingSuggestions(t, agent.role) : [],
      }}
    />
  );
}

/**
 * Who the DM is with, as one button: an agent's page in the panel beside the conversation. The button lies over the face and lines so the name stays the page's heading.
 */
function DmWho({
  agentId,
  label,
  tip,
  children,
}: {
  agentId?: string;
  label: string;
  /** The agent's line whole: under the name it keeps one line and may end in "…". */
  tip?: string;
  children: ReactNode;
}) {
  const openPanel = useOpenAgentPanel();
  const shown = useAgentPanelShown();
  return (
    <div className="dm-who">
      {children}
      {agentId ? (
        <button
          type="button"
          className="profile-trigger dm-who__hit"
          aria-label={label}
          aria-expanded={shown === agentId}
          title={tip}
          onClick={() => openPanel?.(agentId)}
        />
      ) : null}
    </div>
  );
}
