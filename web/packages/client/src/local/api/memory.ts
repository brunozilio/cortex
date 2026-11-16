import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const memoryGroup: NonNullable<HttpApi["memory"]> = {
  list: async () => [],
  listAll: async () => [],
  listTeam: async () => [],
  makeTeam: notAvailable,
  edit: notAvailable,
  setPinned: notAvailable,
  setArchived: notAvailable,
  confirm: notAvailable,
  revert: notAvailable,
};
