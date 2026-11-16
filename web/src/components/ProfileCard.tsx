import type { Actor, Agent, } from "@agenteschat/client";
import {
  agentWorkingIn,
  useAgentDetail,
  useAiStatus,
  useChannels,
  useModels,
  useSendDm,
  useSettings,
} from "@agenteschat/client";
import { type WorkPlaceNames, workDetail } from "@agenteschat/client/lib/agent-work";
import {
  audienceNotice,
  audienceWithMe,
  canTalk,
  seesDetails,
} from "@agenteschat/client/lib/audience";
import { brainLine, } from "@agenteschat/client/lib/brain";
import { errorText } from "@agenteschat/client/lib/errors";
import { clockHour, formatPercent, shortModel } from "@agenteschat/client/lib/format";
import { agentGrammar } from "@agenteschat/client/lib/grammar";
import { can } from "@agenteschat/client/lib/permissions";
import { agentFaceWord, agentWhyKey, isOutOfOffice, } from "@agenteschat/client/lib/presence";
import { apiModelName, autoMakerOf } from "@agenteschat/client/lib/providers";
import { isRightHand, tellsNameStory } from "@agenteschat/client/lib/right-hand";
import { timezoneName } from "@agenteschat/client/lib/routines";
import {
  Avatar,
  Button,
  EmojiPickerPopover,
  Icon,
  Locked,
  Menu,
  Meter,
  PlanMeter,
  Popover,
  ProviderLogo,
  useHoverCard,
} from "@agenteschat/ui";
import { useNavigate } from "@tanstack/react-router";
import type { TFunction } from "i18next";
import { type ReactNode, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AgentAvatar } from "@/components/AgentAvatar";
import { ModelPickerButton } from "@/components/ai/ModelPicker";
import { MenuEntries } from "@/components/ContextMenus";
import { NoAiChip } from "@/components/NoAi";
import { OpensAgent } from "@/components/OpensAgent";
import { useOrg } from "@/components/org-context";
import { LetMeTalkButton, } from "@/components/ReachLines";
import { useEmojiLabels } from "@/components/Reactions";
import { colorOf, ProfileSky } from "@/components/Sky";
import { SeeUsageQueue } from "@/components/UsageQueue";
import { useDirectory } from "@/hooks/useDirectory";
import { type AgentShown, agentLine, useAgentPresence, } from "@/hooks/usePresence";
import { type MenuEntry, tidy } from "@/lib/context-menu";
import { entityMenu } from "@/lib/entity-menu";
import { planMeterText, usageShown } from "@/lib/plan-usage";
import { useToast } from "@/lib/toast";
import { useStreams } from "@/realtime/context";
import { useAgentMenu, usePersonMenu } from "./entity-menus";
import "./profile-card.css";

/** Hover delay before the card opens (mouse and pen only; touch opens on tap). */
export const HOVER_OPEN_MS = 400;

/**
 * Wraps an avatar, author name or mention so it opens the profile card:
 * click, Enter/Space, or ~400 ms of hover with a mouse. Never on touch hover.
 */
export function ProfileTrigger({
  actor,
  label,
  children,
  className,
  decorative,
  describedBy,
  side = "bottom",
}: {
  actor: Actor;
  /** Accessible name of the button ("Perfil de Ada"). */
  label: string;
  /** The id of what the button shows, read after its name (a members row: presence, line, chips). */
  describedBy?: string;
  children: ReactNode;
  className?: string;
  /** A second trigger next to a focusable one (the avatar beside the name): mouse only. */
  decorative?: boolean;
  /** Where the card opens: the members column opens it to its left. */
  side?: "bottom" | "left";
}) {
  const { t } = useTranslation();
  // Hover leaves the focus where it is (the box someone types in), a press or a key brings it in.
  const card = useHoverCard(HOVER_OPEN_MS);
  const { org } = useOrg();
  const dir = useDirectory(org.id);
  const agentEntries = useAgentMenu();
  const personEntries = usePersonMenu();
  const button = useRef<HTMLButtonElement>(null);
  // The trigger takes the focus before the card goes: what opens from the card (the agent's panel) gives it back there.
  const leave = () => {
    if (document.activeElement?.closest(".mcard__frame")) button.current?.focus();
    card.close();
  };
  const menu = entityMenu(() => {
    if (actor.type === "agent") {
      const agent = dir.agentById.get(actor.id);
      return agent ? agentEntries(agent) : [];
    }
    const member = dir.memberById.get(actor.id);
    return member ? personEntries(member, () => button.current?.click()) : [];
  });
  return (
    <Popover
      hover={card}
      label={t("profile.cardLabel")}
      side={side}
      className={`profile-card mcard mcard--${actor.type}`}
      trigger={
        <button
          ref={button}
          type="button"
          {...menu}
          className={className ? `profile-trigger ${className}` : "profile-trigger"}
          aria-label={label}
          aria-describedby={describedBy}
          aria-haspopup="dialog"
          tabIndex={decorative ? -1 : undefined}
          aria-hidden={decorative || undefined}
        >
          {children}
        </button>
      }
    >
      {card.open && (
        // A press inside a card the pointer opened keeps it (its menus and pickers open outside it).
        <div className="mcard__frame" onPointerDownCapture={() => card.open === "hover" && card.onOpenChange(true)}>
          {actor.type === "agent" ? (
            <AgentCard agentId={actor.id} onDone={leave} />
          ) : (
            <PersonCard userId={actor.id} onDone={leave} />
          )}
        </div>
      )}
    </Popover>
  );
}

/** Discord's shape: the banner, its round buttons (a shortcut, then ⋯), the big face over its edge, then the body. */
function CardShell({
  sky,
  face,
  agent,
  act,
  more,
  onDone,
  children,
}: {
  sky: ReactNode;
  face: ReactNode;
  agent?: boolean;
  /** A round button before the ⋯ (an agent's computer). */
  act?: ReactNode;
  /** The ⋯ menu: the same entries as the right click. */
  more?: MenuEntry[];
  /** Closes the card (a ⋯ entry ran). */
  onDone?: () => void;
  children: ReactNode;
}) {
  const { t } = useTranslation();
  const entries = tidy((more ?? []).filter((e) => e.kind !== "header"));
  return (
    <>
      <div className="mcard__banner" aria-hidden="true">
        {sky}
      </div>
      {(act || entries.length > 0) && (
        <div className="mcard__acts">
          {act}
          {entries.length > 0 && (
            <Menu
              label={t("profile.more")}
              side="right"
              align="start"
              trigger={
                <Button
                  size="sm"
                  variant="ghost"
                  iconOnly
                  icon="more"
                  className="mcard__act"
                  aria-label={t("profile.more")}
                />
              }
            >
              <MenuEntries entries={entries} onRun={onDone} />
            </Menu>
          )}
        </div>
      )}
      <div className={agent ? "mcard__face mcard__face--agent" : "mcard__face"}>{face}</div>
      <div className="profile-card__body">{children}</div>
    </>
  );
}

/** "Mensagem para Caio": Enter opens the DM (made if new) with the text sent. */
function MessageBox({ target, label, onSent }: { target: { agentId: string }; label: string; onSent: () => void }) {
  const { t } = useTranslation();
  const { org } = useOrg();
  const navigate = useNavigate();
  const toast = useToast();
  const send = useSendDm(org.id);
  const emojiLabels = useEmojiLabels();
  const field = useRef<HTMLInputElement>(null);
  const [text, setText] = useState("");
  const submit = () => {
    const body = text.trim();
    if (!body || send.isPending) return;
    send.mutate(
      { ...target, body },
      {
        onSuccess: () => {
          setText("");
          onSent();
          void navigate({ to: "/org/$orgId/dm/agent/$agentId", params: { orgId: org.id, agentId: target.agentId } });
        },
        onError: (e) => toast.show(errorText(t, e), "error"),
      },
    );
  };
  // Being deleted, the workspace takes no message (ADR 0088 §7): why, where the box would be.
  if (org.deletion)
    return (
      <p className="small muted ac-iconline" role="status">
        <Icon name="lock" size={16} />
        <span>{t("deleting.composer")}</span>
      </p>
    );
  return (
    <div className="mcard__foot">
      <Locked when={send.isPending}>
        <div className="ac-input-affix mcard__dm">
          <input
            ref={field}
            value={text}
            placeholder={label}
            aria-label={label}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key !== "Enter" || e.nativeEvent.isComposing) return;
              e.preventDefault();
              submit();
            }}
          />
          <EmojiPickerPopover
            labels={emojiLabels}
            side="top"
            align="end"
            onPick={(emoji) => {
              setText((v) => v + emoji);
              field.current?.focus();
            }}
            trigger={
              <button
                type="button"
                className="mcard__emoji"
                aria-label={t("presence.emoji")}
                title={t("presence.emoji")}
              >
                <Icon name="smile" size={18} />
              </button>
            }
          />
        </div>
      </Locked>
    </div>
  );
}

function PersonCard({ userId, onDone }: { userId: string; onDone: () => void }) {
  const { t, i18n } = useTranslation();
  const { org } = useOrg();
  const settings = useSettings(org.id);
  const dir = useDirectory(org.id);
  const member = dir.memberById.get(userId);
  const name = member?.name ?? t("channel.unknownPerson");
  const handle = member?.handle;
  const timezone = member?.timezone ?? settings.data?.timezone;
  let localTime: string | null = null;
  try {
    localTime = new Intl.DateTimeFormat(i18n.language, {
      hour: clockHour(i18n.language),
      minute: "2-digit",
      ...(timezone ? { timeZone: timezone } : {}),
    }).format(new Date());
  } catch {
    localTime = null;
  }
  return (
    <CardShell
      sky={<ProfileSky color="sky" person />}
      face={<Avatar kind="person" size="lg" name={name} imageUrl={member?.avatarUrl ?? member?.image} />}
      more={[]}
      onDone={onDone}
    >
      <div className="list-col mcard__names">
        <span className="profile-card__name">{name}</span>
        {handle && <span className="small muted">@{handle}</span>}
      </div>
      <dl className="profile-card__facts">
        {localTime && (
          <>
            <dt>{t("profile.localTime")}</dt>
            <dd className="tabular">
              {timezone
                ? t("profile.timeIn", { time: localTime, zone: timezoneName(timezone, i18n.language) })
                : localTime}
            </dd>
          </>
        )}
      </dl>
    </CardShell>
  );
}

function AgentCard({ agentId, onDone }: { agentId: string; onDone: () => void }) {
  const { t } = useTranslation();
  const { org } = useOrg();
  const dir = useDirectory(org.id);
  const agent = dir.anyAgent(agentId);
  if (!agent) return <p className="small muted mcard__unknown">{t("channel.unknownAgent")}</p>;
  // No longer in the team (archived or deleted): who it was, nothing to do with it.
  if (agent.former) {
    return (
      <CardShell
        agent
        sky={<ProfileSky color={colorOf(agent)} />}
        face={<AgentAvatar agentId={agent.id} size="lg" name={agent.name} />}
      >
        <div className="list-col mcard__names">
          <span className="profile-card__name">{agent.name}</span>
          <span className="small muted">{t("profile.former")}</span>
        </div>
      </CardShell>
    );
  }
  if (isRightHand(agent)) return <RightHandCardBody agent={agent} onDone={onDone} />;
  return <AgentCardBody agent={agent} onDone={onDone} />;
}

/**
 * The team's right hand (ADR 0137): who it is ("seu braço direito", its one role name everywhere), how it works (acts
 * with your permissions, talks only with you in your DM, replies in channels when called), what thinks for it, then
 * Mensagem, and Configurar for who manages agents.
 */
function RightHandCardBody({ agent, onDone }: { agent: Agent; onDone: () => void }) {
  const { t } = useTranslation();
  const { org } = useOrg();
  const dir = useDirectory(org.id);
  const models = useModels();
  const maker = autoMakerOf(useAiStatus(org.id).data, agent.id);
  const navigate = useNavigate();
  const model = apiModelName(t, agent.model, models.data?.models.find((m) => m.id === agent.model)?.label, maker);
  const thinks = brainLine(t, agent.brain, model, { agents: dir.agents });
  const mark = thinks.mark ?? maker;
  const go = (to: () => Promise<void>) => {
    onDone();
    void to();
  };
  return (
    <CardShell
      agent
      sky={<ProfileSky color={colorOf(agent)} />}
      face={
        <OpensAgent agentId={agent.id} onOpen={onDone}>
          <AgentAvatar agentId={agent.id} size="lg" name={agent.name} />
        </OpensAgent>
      }
    >
      <div className="list-col mcard__names">
        <span className="profile-card__name">{agent.name}</span>
        <span className="small muted">{[`@${agent.handle}`, t("rightHand.label")].join(" · ")}</span>
        {tellsNameStory(agent) && <span className="small muted">{t("rightHand.profile.nameStory")}</span>}
      </div>
      <section className="mcard__section" aria-label={t("rightHand.profile.how")}>
        <span className="mcard__label">{t("rightHand.profile.how")}</span>
        <ul className="mcard__facts">
          <li className="ac-iconline">
            <Icon name="person" size={16} />
            {t("rightHand.profile.permissions")}
          </li>
          <li className="ac-iconline">
            <Icon name="lock" size={16} />
            {t("rightHand.profile.private")}
          </li>
          <li className="ac-iconline">
            <Icon name="message" size={16} />
            {t("rightHand.profile.channels")}
          </li>
        </ul>
      </section>
      <dl className="profile-card__facts">
        <dt>{t("rightHand.profile.model")}</dt>
        <dd>
          <span className="profile-card__brain ac-iconline">
            {mark && <ProviderLogo id={mark} size="xs" />}
            {thinks.text || shortModel(model)}
          </span>
        </dd>
      </dl>
      <div className="profile-card__actions profile-card__actions--split">
        <Button
          variant="primary"
          icon="message"
          onClick={() =>
            go(() => navigate({ to: "/org/$orgId/dm/agent/$agentId", params: { orgId: org.id, agentId: agent.id } }))
          }
        >
          {t("rightHand.profile.message")}
        </Button>
        {can(org, "manage_agents") && (
          <Button
            icon="cog"
            onClick={() =>
              go(() => navigate({ to: "/org/$orgId/agents/$agentId", params: { orgId: org.id, agentId: agent.id } }))
            }
          >
            {t("rightHand.profile.configure")}
          </Button>
        )}
      </div>
    </CardShell>
  );
}

function AgentCardBody({ agent, onDone }: { agent: Agent; onDone: () => void }) {
  const { t, i18n } = useTranslation();
  const { org, me } = useOrg();
  const dir = useDirectory(org.id);
  const channels = useChannels(org.id).data;
  const agentEntries = useAgentMenu();
  const seesSpend = can(org, "view_usage") || agent.ownerUserId === me.user.id;
  const details = seesDetails(agent);
  const g = agentGrammar(agent.name);
  const owner = agent.ownerUserId ? dir.memberById.get(agent.ownerUserId)?.name : null;
  // Its presence from its work and its line (ADR 0084): its own words or what it is doing now, and where.
  const shown = useAgentPresence(agent.id);
  const line = agentLine(t, shown?.line ?? null, i18n.language);
  const workingIn = useStreams((s) => agentWorkingIn(s, agent.id));
  const busy = workingIn !== null;
  const here = workingIn ? channels?.find((c) => c.id === workingIn)?.name : undefined;
  return (
    <CardShell
      agent
      sky={<ProfileSky color={colorOf(agent)} busy={busy} />}
      face={
        <OpensAgent agentId={agent.id} onOpen={onDone}>
          <AgentAvatar
            agentId={agent.id}
            size="lg"
            name={agent.name}
            presence={shown?.presence ?? null}
            presenceLabel={shown ? agentFaceWord(t, shown.presence) : undefined}
            asleep={isOutOfOffice(shown?.line)}
          />
        </OpensAgent>
      }
      more={agentEntries(agent, [])}
      onDone={onDone}
    >
      <div className="list-col mcard__names">
        <span className="profile-card__name">{agent.name}</span>
        <span className="small muted">
          {[
            agent.handle ? `@${agent.handle}` : null,
            agent.role ? t("issues.agentRole", { role: agent.role }) : t("profile.agent"),
          ]
            .filter(Boolean)
            .join(" · ")}
        </span>
      </div>
      <NoAiChip agentId={agent.id} />
      {line && shown?.line && (
        <div className={line.icon ? "profile-card__status ac-iconline" : "profile-card__status"}>
          <span className="profile-card__status-emoji" aria-hidden="true">
            {line.icon ? <Icon name={line.icon} size={16} /> : line.emoji}
          </span>
          <span>
            {line.text}
            <small>
              {here
                ? t("profile.nowIn", { channel: here })
                : shown.line.by === "self"
                  ? t("presence.bySelf", { name: agent.name })
                  : t("presence.byAuto")}
            </small>
            {/* Out of office with conversations waiting (ADR 0118): what it picks up when it is back, in order. */}
            {isOutOfOffice(shown.line) && shown.line.count ? <SeeUsageQueue agentId={agent.id} /> : null}
          </span>
        </div>
      )}
      <dl className="profile-card__facts">
        {shown && (
          <>
            <dt>{t("profile.presence")}</dt>
            <dd>
              {[
                agentFaceWord(t, shown.presence),
                agentWhy(
                  t,
                  shown,
                  {
                    agentName: (id) => dir.agentById.get(id)?.name,
                    channelName: (id) => channels?.find((c) => c.id === id)?.name,
                  },
                  i18n.language,
                ),
              ]
                .filter(Boolean)
                .join(" · ")}
            </dd>
          </>
        )}
        <dt>{t("profile.owner")}</dt>
        <dd>{owner ?? t("profile.noOwner")}</dd>
        {details && (
          <>
            <dt>{t("profile.model")}</dt>
            <dd>
              <ModelPickerButton agent={agent} size="row" className="profile-card__brain" />
            </dd>
          </>
        )}
      </dl>
      {seesSpend && details && <Spend agent={agent} locale={i18n.language} />}
      {canTalk(agent) ? (
        <MessageBox target={{ agentId: agent.id }} label={t("home.ask.label", g)} onSent={onDone} />
      ) : (
        <>
          <p className="small muted">{audienceNotice(t, agent, (id) => dir.memberById.get(id)?.name, dir.roleName)}</p>
          <div className="profile-card__actions">
            {audienceWithMe(org, agent, me.user.id) && <LetMeTalkButton agent={agent} />}
          </div>
        </>
      )}
    </CardShell>
  );
}

/**
 * What follows an agent's face word on its card (ADR 0084 addendum): whom it helps or where it works ("ajudando a
 * Ada"), what it waits on you for ("sua aprovação"), why it is out or paused; free, only a return it set ("retorno às
 * 00:32").
 */
function agentWhy(t: TFunction, shown: AgentShown, names: WorkPlaceNames, locale: string): string | null {
  const key = agentWhyKey(shown.presence, shown.noAi);
  if (key) return t(`presence.agentWhy.${key}`);
  if (shown.presence === "online" && shown.work?.state !== "later") return null;
  return workDetail(t, shown.work, names, locale);
}

function Spend({ agent, locale }: { agent: Agent; locale: string }) {
  const { t } = useTranslation();
  const { org } = useOrg();
  const detail = useAgentDetail(org.id, agent.id);
  if (!detail.data) return null;
  // A percentage of its budget, never money (owners who aren't admins see this too); on a plan, its limits first
  // and the budget only once it spent through the API (ADR 0063 §5).
  const used = detail.data.budgetUsedPercent;
  const shown = usageShown(agent.brain, used);
  if (!shown.plan && !shown.budget) return null;
  return (
    <div className="stack-4" style={{ gap: 4 }}>
      {shown.plan && (
        <PlanMeter compact usage={shown.plan.usage} text={planMeterText(t, locale)} label={shown.plan.account} />
      )}
      {shown.budget && (
        <span className="small">{t("profile.budgetUsed", { percent: formatPercent(locale, used) })}</span>
      )}
      {shown.budget && !shown.plan && <Meter value={Math.min(100, used)} max={100} label={t("profile.spendLabel")} />}
    </div>
  );
}
