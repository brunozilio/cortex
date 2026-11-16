import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const dmsGroup: NonNullable<HttpApi["dms"]> = {
  list: async () => [],
  open: notAvailable,
  markRead: notAvailable,
  reads: async () => [],
};
