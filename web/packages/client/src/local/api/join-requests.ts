import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const joinRequestsGroup: NonNullable<HttpApi["joinRequests"]> = {
  list: async () => [],
  approve: notAvailable,
  deny: notAvailable,
  suggest: notAvailable,
  mine: async () => [],
  cancel: notAvailable,
};
