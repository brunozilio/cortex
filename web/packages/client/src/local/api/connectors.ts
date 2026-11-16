import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const connectorsGroup: NonNullable<HttpApi["connectors"]> = {
  catalog: async () => ({ connectors: [], builtinScopes: [] }),
  list: async () => [],
  connectWithKey: notAvailable,
  startOAuth: notAvailable,
  composioUsage: notFound,
  completeOAuth: notAvailable,
  rename: notAvailable,
  disconnect: notAvailable,
};
