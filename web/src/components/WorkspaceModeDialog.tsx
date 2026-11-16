import type { OrgSettings, WorkspaceMode } from "@agenteschat/client";
import { useUpdateSettings } from "@agenteschat/client";
import { modeSwitchLines } from "@agenteschat/client/lib/personal-view";
import { Button, Dialog, DialogBody, DialogFooter, Field, Input, Locked, } from "@agenteschat/ui";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { DeletionLine } from "@/components/DeleteWorkspace";
import { useOrg } from "@/components/org-context";
import { PurposeScene } from "@/components/PurposeScene";
import { usePurposeLife } from "@/components/purpose-life";
import { useDirectory } from "@/hooks/useDirectory";
import { useToast } from "@/lib/toast";
// The drawing's own styles (the onboarding's first question draws the same two scenes).
import "./workspace-mode.css";
import "./workspace-mode.css";

/**
 * Switching a workspace between team and personal (ADR 0159 §4), for who configures it. Going to a team's it says what
 * comes into sight and asks the workspace's name; going to personal it says what changes and, with other members,
 * that they see it too. What stays is said apart. Over the words, the drawing of the mode it goes to (the onboarding's
 * `PurposeScene`: the person with their agents around them, or a team's chart), waking as the dialog opens. Only the
 * view changes: every agent, conversation, file and
 * permission stays as it is, and every member's apps reshape on the server's `org.mode`.
 */
export function WorkspaceModeDialog({
  open,
  to,
  onClose,
  onSwitched,
}: {
  open: boolean;
  /** The mode it switches to. */
  to: WorkspaceMode;
  onClose: () => void;
  /** Once the server switched it, with the workspace's settings (the name it has now). */
  onSwitched?: (settings: OrgSettings) => void;
}) {
  const { t } = useTranslation();
  const { org, me } = useOrg();
  const toast = useToast();
  const update = useUpdateSettings(org.id);
  const others = useDirectory(org.id).members.filter((m) => m.userId !== me.user.id).length;
  const [name, setName] = useState(org.name);
  const nameField = useRef<HTMLInputElement>(null);
  const team = to === "team";
  const words = team ? "toTeam" : "toPersonal";
  // The scene wakes with the dialog, as the onboarding's does when its card is chosen.
  const life = usePurposeLife(to, open);
  // Each opening starts from the workspace's name as it is now.
  useEffect(() => {
    if (open) setName(org.name);
  }, [open, org.name]);
  const named = name.trim();
  const ready = !team || named.length > 0;
  const submit = () => {
    if (!ready || update.isPending) return;
    const patch = team ? { mode: to, ...(named !== org.name ? { name: named } : {}) } : { mode: to };
    update.mutate(patch, {
      onSuccess: (settings) => {
        toast.show(team ? t("personalMode.toTeam.done", { name: settings.name }) : t("personalMode.toPersonal.done"));
        onSwitched?.(settings);
        onClose();
      },
    });
  };

  return (
    <Dialog
      bare
      className="mode-dialog"
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t(`personalMode.${words}.title`)}
      closeLabel={t("common.close")}
      width={480}
      initialFocus={team ? nameField : undefined}
    >
      <div className="mode-dialog__hero">
        <PurposeScene
          kind={to}
          chosen
          you={{ name: me.user.name || me.user.email, image: me.user.image ?? null }}
          life={life}
        />
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="close"
          className="mode-dialog__close"
          aria-label={t("common.close")}
          onClick={onClose}
        />
      </div>
      {/* The dialog's name is its hidden title; this is the same sentence, drawn. */}
      <div className="mode-dialog__head" aria-hidden="true">
        <p className="ac-dialog__title">{t(`personalMode.${words}.title`)}</p>
      </div>
      <p className="ac-dialog__desc">{t(`personalMode.${words}.lead`)}</p>
      <form
        style={{ display: "contents" }}
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <Locked when={update.isPending}>
          <DialogBody>
            <ul className="deletion-lines">
              {modeSwitchLines(to, others).map((line) => (
                <DeletionLine key={line.key} icon={line.icon}>
                  {t(line.key, { count: line.count })}
                </DeletionLine>
              ))}
            </ul>
            <p className="small muted deletion-note">{t(`personalMode.${words}.stays`)}</p>
            {team && (
              <Field label={t("personalMode.toTeam.name")}>
                <Input
                  ref={nameField}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  onFocus={(e) => e.currentTarget.select()}
                  autoComplete="off"
                  spellCheck={false}
                />
              </Field>
            )}
          </DialogBody>
        </Locked>
        <DialogFooter>
          <Button type="button" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button type="submit" variant="primary" disabled={!ready} loading={update.isPending}>
            {t(`personalMode.${words}.confirm`)}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

