import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const proposalsGroup: NonNullable<HttpApi["proposals"]> = {
  list: async () => [],
  accept: notAvailable,
  refuse: notAvailable,
  ask: notAvailable,
};
