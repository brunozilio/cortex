import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const threadsGroup: NonNullable<HttpApi["threads"]> = {
  list: async () => ({ threads: [], nextCursor: null }),
  inChannel: async () => ({ threads: [], nextCursor: null }),
  get: notFound,
  create: notAvailable,
  update: notAvailable,
  remove: notAvailable,
  act: notAvailable,
};
