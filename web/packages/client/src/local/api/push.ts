import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const pushGroup: NonNullable<HttpApi["push"]> = {
  webKey: async () => null,
  register: notAvailable,
  unregister: notAvailable,
  stale: notAvailable,
};
