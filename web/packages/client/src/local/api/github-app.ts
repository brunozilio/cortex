import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const githubAppGroup: NonNullable<HttpApi["githubApp"]> = {
  info: async () => ({ installable: false, installations: [] }),
  install: notAvailable,
  complete: notAvailable,
  remove: notAvailable,
  startManifest: notAvailable,
  removeSettings: notAvailable,
  repositories: async () => ({
    connected: false,
    accounts: [],
    truncated: false,
  }),
  access: async () => ({ orgs: null, manageUrl: null }),
};
