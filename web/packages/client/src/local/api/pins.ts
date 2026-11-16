import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const pinsGroup: NonNullable<HttpApi["pins"]> = {
  list: async () => [],
  pin: notAvailable,
  unpin: notAvailable,
};
