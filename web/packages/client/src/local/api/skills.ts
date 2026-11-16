import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const skillsGroup: NonNullable<HttpApi["skills"]> = {
  list: async () => [],
  get: notFound,
  create: notAvailable,
  import: notAvailable,
  importPicked: notAvailable,
  update: notAvailable,
  reimport: notAvailable,
  remove: notAvailable,
  forAgent: async () => [],
  setForAgent: notAvailable,
  learned: async () => [],
  revert: notAvailable,
  updateFromSource: notAvailable,
  approveUpdate: notAvailable,
  dismissUpdate: notAvailable,
  reopenUpdate: notAvailable,
  undoUpdate: notAvailable,
  restore: notAvailable,
  settings: async () => ({ agentsImprove: false }),
  saveSettings: notAvailable,
};
