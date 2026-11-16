import type { HttpApi } from "../../api/client";
import { notAvailable, notAvailableError, notFound } from "../unavailable";

export const artifactsGroup: NonNullable<HttpApi["artifacts"]> = {
  list: async () => [],
  page: async () => ({ artifacts: [], nextCursor: null }),
  get: notFound,
  keep: notAvailable,
  reply: notAvailable,
  blockState: notFound,
  setBlockState: notAvailable,
  drawBlock: notAvailable,
  poll: notFound,
  vote: notAvailable,
  choices: notFound,
  answerChoices: notAvailable,
  versions: async () => [],
  download: notFound,
  fileUrl: () => {
    throw notAvailableError();
  },
  create: notAvailable,
  upload: notAvailable,
  uploadVersion: notAvailable,
  fileBlob: notAvailable,
  update: notAvailable,
  setSharing: notAvailable,
  remove: notAvailable,
  trash: async () => [],
  restore: notAvailable,
  purge: notAvailable,
};
