import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const repositoriesGroup: NonNullable<HttpApi["repositories"]> = {
  get: async () => [],
  set: notAvailable,
  merge: async () => [],
  setMerge: notAvailable,
};
