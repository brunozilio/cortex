import type { HttpApi } from "../../api/client";
import { notAvailable, notAvailableError } from "../unavailable";

export const importsGroup: NonNullable<HttpApi["imports"]> = {
  apply: notAvailable,
  sources: async () => [],
  copyFile: notAvailable,
  exportUrl: () => {
    throw notAvailableError();
  },
};
