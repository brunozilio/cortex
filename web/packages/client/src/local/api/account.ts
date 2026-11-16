import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const accountGroup: NonNullable<HttpApi["account"]> = {
  authConfig: async () => ({
    socialProviders: { google: false, github: false },
    signupOpen: false,
    firstRun: false,
  }),
  signInOptions: notAvailable,
  me: async () => null,
  unread: async () => null,
  deviceAccounts: async () => [],
  switchDeviceAccount: notAvailable,
  signOutDeviceAccount: notAvailable,
  updateMe: notAvailable,
  setWorkArea: notAvailable,
  phoneSignIn: notAvailable,
  addPassword: notAvailable,
  deletionPlan: async () => ({
    closes: [],
    leaves: [],
    soleOwner: [],
    computers: [],
    staysBehind: [],
    serverOwner: false,
  }),
  deletionCode: notAvailable,
  delete: notAvailable,
  restore: notAvailable,
  changeEmail: notAvailable,
  undoEmailChange: notAvailable,
};
