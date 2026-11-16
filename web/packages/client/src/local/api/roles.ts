import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const rolesGroup: NonNullable<HttpApi["roles"]> = {
  list: async () => ({ roles: [], areas: [], colors: [], icons: [] }),
  create: notAvailable,
  update: notAvailable,
  remove: notAvailable,
  reorder: notAvailable,
  setFor: notAvailable,
  setRow: notAvailable,
  mailLoss: notAvailable,
};
