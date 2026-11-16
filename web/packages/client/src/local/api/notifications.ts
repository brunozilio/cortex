import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const notificationsGroup: NonNullable<HttpApi["notifications"]> = {
  list: async () => ({
    items: [],
    counts: { waiting: 0, all: 0, mentions: 0, unread: 0, reactions: 0 },
    nextCursor: null,
  }),
  count: async () => ({ unread: 0, waiting: 0 }),
  waiting: async () => ({ items: [], count: 0 }),
  read: notAvailable,
  done: notAvailable,
  snooze: notAvailable,
  readAll: notAvailable,
  doneAll: notAvailable,
  prefs: async () => ({ categories: [], phones: 0 }),
  setPref: notAvailable,
  askAdmins: notAvailable,
};
