import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const scopesGroup: NonNullable<HttpApi["scopes"]> = {
  get: async () => [],
  set: notAvailable,
};
