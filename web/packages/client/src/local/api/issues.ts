import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const issuesGroup: NonNullable<HttpApi["issues"]> = {
  list: async () => [],
  get: notFound,
  create: notAvailable,
  update: notAvailable,
  delete: notAvailable,
  setCriterion: notAvailable,
  share: notAvailable,
  setAudience: notAvailable,
  activity: async () => [],
  comment: notAvailable,
  askAgent: notAvailable,
  conversation: notAvailable,
  columns: async () => [],
  addColumn: notAvailable,
  updateColumn: notAvailable,
  deleteColumn: notAvailable,
  projects: async () => ({ projects: [], unfiled: 0 }),
  createProject: notAvailable,
  updateProject: notAvailable,
  deleteProject: notAvailable,
};
