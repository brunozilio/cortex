import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const devicesGroup: NonNullable<HttpApi["devices"]> = {
  list: async () => ({ devices: [], requests: [] }),
  updateComputer: notAvailable,
  removeComputer: notAvailable,
  updatePhone: notAvailable,
  forgetPhone: notAvailable,
  stopPhone: notAvailable,
  createLink: notAvailable,
  link: async () => ({ status: "waiting" }),
  approveRequest: notAvailable,
  denyRequest: notAvailable,
  decideAccess: notAvailable,
  deviceWait: notAvailable,
  startWifiPairing: notAvailable,
  wifiPairing: notFound,
  cancelWifiPairing: notAvailable,
  lookForPhones: notAvailable,
  wifiIphones: async () => ({ available: [] }),
  pickIphoneOverWifi: notAvailable,
  startXcodeFix: notAvailable,
  xcodeFix: async () => ({ fix: null }),
  cancelXcodeFix: notAvailable,
  openXcode: notAvailable,
};
