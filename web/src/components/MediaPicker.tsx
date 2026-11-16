import type { MessageMedia, StickerMood } from "@agenteschat/client";
import { useSettings, } from "@agenteschat/client";
import {
  mediaTabsOf,
  stickerLabel,
  stickerMedia,
  stickerWords,
} from "@agenteschat/client/lib/media";
import {
  type PickerAgent,
  type PickerTab,
  StickerPicker,
  type StickerPickerLabels,
} from "@agenteschat/ui";
import { useMemo, } from "react";
import { useTranslation } from "react-i18next";
import { AgentAvatar } from "@/components/AgentAvatar";
import { useOrg } from "@/components/org-context";
import { useDirectory } from "@/hooks/useDirectory";
import { lookOf } from "@/lib/look";

/** A sticker's side in the picker (CSS pixels): four to a row. */
const PICKER_STICKER = 72;

/** What the composer needs for its picker with tabs: the tabs, their names and the emoji button's. */
export interface MediaTabs {
  tabs: PickerTab[];
  labels: { dialog: string; tabs: string };
  button: string;
}

/**
 * The composer picker's GIFs and stickers tabs (ADR 0091), each a click that sends (`onSend`) in the conversation
 * the composer writes to. GIFs only where this server has GIF search and the workspace has them on; stickers where
 * the workspace has them on and has agents. Neither: null, and the button is the emoji picker it always was.
 */
export function useMediaTabs(onSend: (media: MessageMedia) => void): MediaTabs | null {
  const { t } = useTranslation();
  const { org } = useOrg();
  const settings = useSettings(org.id).data;
  const dir = useDirectory(org.id);
  const open = mediaTabsOf(settings, dir.agents.length, false);
  const stickers = open.includes("stickers");
  return useMemo(() => {
    const tabs: PickerTab[] = [];
    if (stickers) {
      tabs.push({
        id: "stickers",
        label: t("media.picker.stickers"),
        render: ({ close, autoFocus }) => (
          <StickerTab
            autoFocus={autoFocus}
            onPick={(media) => {
              onSend(media);
              close();
            }}
          />
        ),
      });
    }
    if (tabs.length === 0) return null;
    return {
      tabs,
      labels: { dialog: t("media.picker.button"), tabs: t("media.picker.tabs") },
      button: t("media.picker.button"),
    };
  }, [stickers, onSend, t]);
}

/** The words of the stickers tab. */
function useStickerLabels(): StickerPickerLabels {
  const { t } = useTranslation();
  return useMemo(
    () => ({
      search: t("media.stickers.search"),
      clearSearch: t("media.stickers.clear"),
      empty: t("media.stickers.empty"),
      foot: t("media.stickers.foot"),
      ...stickerWords(t),
      sticker: (name: string, mood: StickerMood) => stickerLabel(t, name, mood),
    }),
    [t],
  );
}

/** The stickers tab: every agent of the team the person sees, with its six moods. */
function StickerTab({ autoFocus, onPick }: { autoFocus: boolean; onPick: (media: MessageMedia) => void }) {
  const { org } = useOrg();
  const dir = useDirectory(org.id);
  const labels = useStickerLabels();
  const agents: PickerAgent[] = useMemo(
    () =>
      dir.agents.map((a) => ({
        id: a.id,
        name: a.name,
        appearance: lookOf(a),
        avatar: <AgentAvatar agentId={a.id} name={a.name} size="xs" />,
      })),
    [dir.agents],
  );
  return (
    <StickerPicker
      labels={labels}
      agents={agents}
      size={PICKER_STICKER}
      autoFocus={autoFocus}
      onPick={(agent, mood) => onPick(stickerMedia(agent.id, mood))}
    />
  );
}
