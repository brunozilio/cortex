import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const storageGroup: NonNullable<HttpApi["storage"]> = {
  get: async () => ({
    kind: "agentes",
    s3: null,
    migration: null,
    platform: { kind: "bucket", folder: null },
  }),
  test: notAvailable,
  save: notAvailable,
  migrate: notAvailable,
  migration: async () => null,
};
