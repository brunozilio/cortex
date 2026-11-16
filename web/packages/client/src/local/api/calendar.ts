import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const calendarGroup: NonNullable<HttpApi["calendar"]> = {
  range: async () => ({
    events: [],
    routines: [],
    deadlines: [],
    proposals: [],
  }),
  create: notAvailable,
  confirmProposal: notAvailable,
  dismissProposal: notAvailable,
  respond: notAvailable,
  update: notAvailable,
  remove: notAvailable,
};
