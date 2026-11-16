import { useAuthConfig, useComputerSetup, useMachines } from "@agenteschat/client";
import { computerEdition } from "@agenteschat/client/lib/computer";
import { namesComputers } from "@agenteschat/client/lib/machines";

export function useComputerEdition(orgId: string) {
  return computerEdition(useComputerSetup(orgId).data, useAuthConfig().data?.workspaceComputer);
}

export function useNamesComputers(orgId: string) {
  return namesComputers(useMachines(orgId).data);
}
