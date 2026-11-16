import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const browserGroup: NonNullable<HttpApi["browser"]> = {
  settings: async () => ({
    allowedDomains: null,
    available: false,
    reason: null,
    detail: null,
    machine: null,
  }),
  setDomains: notAvailable,
  agentHandoff: async () => null,
  agentControl: async () => false,
  handoff: notFound,
  takeControl: notAvailable,
  done: notAvailable,
  failed: notAvailable,
  cancel: notAvailable,
  takeover: notAvailable,
  open: notAvailable,
  stop: notAvailable,
  restart: notAvailable,
};
