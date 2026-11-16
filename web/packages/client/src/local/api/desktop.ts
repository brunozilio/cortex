import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const desktopGroup: NonNullable<HttpApi["desktop"]> = {
  status: async () => ({
    available: false,
    reason: null,
    detail: null,
    running: false,
    startedAt: null,
    controllerUserId: null,
    handoff: null,
    viewers: 0,
    canControl: false,
    held: false,
  }),
  picture: async () => ({ picture: null, reason: null }),
  holds: async () => ({ holds: [], canStop: false }),
  stopHold: notAvailable,
};
