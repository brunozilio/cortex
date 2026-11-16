import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const vaultGroup: NonNullable<HttpApi["vault"]> = {
  overview: async () => ({
    items: [],
    connections: [],
    sources: [],
    canManageTeam: false,
  }),
  item: notFound,
  create: notAvailable,
  update: notAvailable,
  remove: notAvailable,
  importCsv: notAvailable,
  importKeys: notAvailable,
  move: notAvailable,
  connect: notAvailable,
  updateConnection: notAvailable,
  syncConnection: notAvailable,
  removeConnection: notAvailable,
};
