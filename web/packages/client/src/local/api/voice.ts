import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const voiceGroup: NonNullable<HttpApi["voice"]> = {
  allowed: async () => ({ allowed: false }),
  send: notAvailable,
};
