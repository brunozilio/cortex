import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const accessGroup: NonNullable<HttpApi["access"]> = {
  get: async () => ({ all: false, builtins: [], connectors: [] }),
  set: notAvailable,
};
