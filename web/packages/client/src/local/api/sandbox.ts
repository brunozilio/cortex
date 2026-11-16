import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const sandboxGroup: NonNullable<HttpApi["sandbox"]> = {
  settings: async () => ({
    enabled: false,
    execTimeoutSeconds: 0,
    maxTimeoutSeconds: 0,
    requireApproval: false,
  }),
  saveSettings: notAvailable,
  computer: async () => ({ target: { kind: "none" } }),
  setTarget: notAvailable,
};
