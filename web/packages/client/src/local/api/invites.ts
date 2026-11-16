import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const invitesGroup: NonNullable<HttpApi["invites"]> = {
  createLink: notAvailable,
  editLink: notAvailable,
  links: async () => [],
  turnOff: notAvailable,
  renewLink: notAvailable,
  sendCode: notAvailable,
  checkCode: notAvailable,
  ask: notAvailable,
  claim: notAvailable,
  requestNew: notAvailable,
};
