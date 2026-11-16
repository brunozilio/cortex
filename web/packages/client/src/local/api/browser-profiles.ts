import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const browserProfilesGroup: NonNullable<HttpApi["browserProfiles"]> = {
  list: async () => [],
  create: notAvailable,
  update: notAvailable,
  remove: notAvailable,
  takeover: notAvailable,
  handback: notAvailable,
  open: notAvailable,
  setDefault: notAvailable,
  request: notAvailable,
};
