import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const actionCardsGroup: NonNullable<HttpApi["actionCards"]> = {
  get: notFound,
  decide: notAvailable,
};
