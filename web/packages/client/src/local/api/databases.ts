import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const databasesGroup: NonNullable<HttpApi["databases"]> = {
  list: async () => [],
  connect: notAvailable,
  update: notAvailable,
  disconnect: notAvailable,
  tables: async () => [],
  serverDatabases: async () => [],
  agent: async () => [],
  setAgent: notAvailable,
};
