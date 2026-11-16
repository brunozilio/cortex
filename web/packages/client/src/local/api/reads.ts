import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const readsGroup: NonNullable<HttpApi["reads"]> = {
  mute: notAvailable,
  markThreadRead: notAvailable,
  unreadThreads: async () => 0,
  unreadTasks: async () => [],
};
