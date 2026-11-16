import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const appsGroup: NonNullable<HttpApi["apps"]> = {
  consent: notFound,
  decide: notAvailable,
  list: notFound,
  disconnect: notAvailable,
  createToken: notAvailable,
  revokeToken: notAvailable,
};
