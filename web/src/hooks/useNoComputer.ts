import { useComputerSetup } from "@agenteschat/client";
import { type NoComputer, noComputerChoice, setupPending, setupView } from "@agenteschat/client/lib/computer";
import { modeKey } from "@agenteschat/client/lib/workspace-mode";
import { useTranslation } from "react-i18next";
import { useOrg } from "@/components/org-context";

export const NO_COMPUTER = "none";

export function useNoComputer() {
  const { t } = useTranslation();
  const { org } = useOrg();
  const setupQ = useComputerSetup(org.id);
  const setup = setupView(setupQ.data, setupQ.dataUpdatedAt);
  return {
    setup,
    pending: setupPending(setup),
    choice: (current: boolean) => noComputerChoice(setup, current),
    label: (choice: NoComputer) =>
      choice === "workspace" ? t(modeKey(org, "computerSetup.title"), { org: org.name }) : t("computer.target.none"),
  };
}
