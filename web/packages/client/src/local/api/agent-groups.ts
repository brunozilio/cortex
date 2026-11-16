import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const agentGroupsGroup: NonNullable<HttpApi["agentGroups"]> = {
  list: async () => [],
  create: notAvailable,
  update: notAvailable,
  remove: notAvailable,
  move: notAvailable,
};
