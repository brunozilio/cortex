import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const secretsGroup: NonNullable<HttpApi["secrets"]> = {
  get: notFound,
  deliver: notAvailable,
  decline: notAvailable,
  list: async () => [],
  remove: notAvailable,
  saveToVault: notAvailable,
};
