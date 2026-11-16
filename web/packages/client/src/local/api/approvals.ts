import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const approvalsGroup: NonNullable<HttpApi["approvals"]> = {
  list: async () => [],
  get: notFound,
  approve: notAvailable,
  refuse: notAvailable,
  cancel: notAvailable,
  decideMany: notAvailable,
  rules: async () => [],
  removeRule: notAvailable,
  covers: async () => ({ cover: null, covering: [] }),
  setCover: notAvailable,
  endCover: notAvailable,
  rightHand: notFound,
  setRightHand: notAvailable,
  undoRightHand: notAvailable,
};
