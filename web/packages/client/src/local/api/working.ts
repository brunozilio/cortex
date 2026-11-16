import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const workingGroup: NonNullable<HttpApi["working"]> = {
  list: async () => [],
  taskManager: async () => ({ tasks: [], ended: [], activity: {} }),
  taskSteps: notFound,
  continueTasks: notAvailable,
};
