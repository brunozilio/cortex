import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const remindersGroup: NonNullable<HttpApi["reminders"]> = {
  create: notAvailable,
  list: async () => [],
  cancel: notAvailable,
};
