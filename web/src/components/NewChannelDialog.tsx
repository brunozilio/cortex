import { keys, useChannels, } from "@agenteschat/client";
import { useHeldWhile } from "@agenteschat/client/lib/hold";
import { can } from "@agenteschat/client/lib/permissions";
import { modeKey } from "@agenteschat/client/lib/workspace-mode";
import {
  Button,
  Dialog,
  DialogBody,
  DialogFooter,
  Field,
  Input,
  Locked,
} from "@agenteschat/ui";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useApi } from "@/api/context";
import { CategorySelect } from "./ChannelCategoryDialogs";
import { memberOf, PeoplePicker, type PickKey } from "./MemberPicker";
import { useOrg } from "./org-context";

/** Same rule as the backend (CHANNEL_NAME_PATTERN): lower case, digits, - or _. */
export const CHANNEL_NAME = /^[a-z0-9][a-z0-9_-]{0,79}$/;

/** What people type → a channel name ("Marketing Brasil" → "marketing-brasil"). */
export const toChannelName = (value: string) =>
  value
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/\s+/g, "-")
    .replace(/[^a-z0-9_-]/g, "")
    .slice(0, 80);

export function channelNameError(name: string, taken: Set<string>): "invalid" | "taken" | null {
  if (!CHANNEL_NAME.test(name)) return "invalid";
  if (taken.has(name)) return "taken";
  return null;
}

/**
 * "Criar canal", in one column: name, topic, sidebar category, who can join, then the initial members (people and
 * agents: search and a list that scrolls in its own box). The footer, with the count and Criar, stays in view. A
 * personal workspace calls it a group conversation, and alone leaves out who can join while it is the default, open to
 * everyone here (ADR 0159): the same channel either way.
 */
export function NewChannelDialog({
  open,
  onOpenChange,
  category = null,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Opened from a category's +: it starts chosen. */
  category?: string | null;
}) {
  const { t } = useTranslation();
  const { org } = useOrg();
  const api = useApi();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const channels = useChannels(org.id);
  const [name, setName] = useState("");
  const [topic, setTopic] = useState("");
  const [categoryId, setCategoryId] = useState<string | null>(category);
  const [selected, setSelected] = useState<Set<PickKey>>(new Set());
  const [touched, setTouched] = useState(false);
  const create = useMutation({
    mutationFn: async () => {
      const channel = await api.chat.createChannel(org.id, {
        name,
        topic: topic.trim() || undefined,
        ...(categoryId ? { categoryId } : {}),
      });
      for (const key of selected) await api.chat.addMember(org.id, channel.id, memberOf(key));
      return channel;
    },
    onSuccess: async (channel) => {
      await qc.invalidateQueries({ queryKey: keys.channels(org.id) });
      void qc.invalidateQueries({ queryKey: keys.channelCategories(org.id) });
      onOpenChange(false);
      await navigate({ to: "/org/$orgId/c/$channelId", params: { orgId: org.id, channelId: channel.id } });
    },
  });
  useEffect(() => {
    if (!open) return;
    setName("");
    setTopic("");
    setCategoryId(category);
    setSelected(new Set());
    setTouched(false);
    create.reset();
  }, [open, category, create.reset]);
  const taken = useHeldWhile(new Set((channels.data ?? []).map((c) => c.name)), create.isPending || create.isSuccess);
  const error = channelNameError(name, taken);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={t(modeKey(org, "newChannel.title"))}
      closeLabel={t("common.close")}
    >
      <form
        style={{ display: "contents" }}
        onSubmit={(e) => {
          e.preventDefault();
          setTouched(true);
          if (!error) create.mutate();
        }}
      >
        <Locked when={create.isPending}>
          <DialogBody>
            <Field
              label={t("newChannel.name")}
              id="ch-name"
              example={t("newChannel.nameHelp", { name: name || "canal" })}
              error={
                touched || (name && error === "taken")
                  ? error === "invalid"
                    ? t("newChannel.nameInvalid")
                    : error === "taken"
                      ? t(modeKey(org, "newChannel.nameTaken"), { name })
                      : undefined
                  : undefined
              }
            >
              <Input
                value={name}
                onChange={(e) => setName(toChannelName(e.target.value))}
                placeholder={t(modeKey(org, "newChannel.namePlaceholder"))}
                autoFocus
              />
            </Field>
            <Field label={t("newChannel.topic")} id="ch-topic">
              <Input
                value={topic}
                onChange={(e) => setTopic(e.target.value)}
                placeholder={t("newChannel.topicPlaceholder")}
              />
            </Field>
            <Field
              label={t("channelCategories.category")}
              id="ch-category"
              hint={{
                name: t("common.about", { label: t("channelCategories.category") }),
                text: t("channelCategories.categoryHelp"),
              }}
            >
              <CategorySelect value={categoryId} onChange={setCategoryId} canCreate={can(org, "manage_channels")} />
            </Field>
            <PeoplePicker
              label={t(modeKey(org, "newChannel.members"))}
              searchLabel={t("newChannel.membersSearch")}
              emptyText={(q) => t("newChannel.membersEmpty", { q })}
              selected={selected}
              onChange={setSelected}
              channel
            />
          </DialogBody>
        </Locked>
        <DialogFooter bar note={t("newChannel.selected", { count: selected.size })}>
          <Button onClick={() => onOpenChange(false)}>{t("common.cancel")}</Button>
          <Button type="submit" variant="primary" disabled={create.isPending} loading={create.isPending}>
            {t("newChannel.create", { name: name || "canal" })}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
