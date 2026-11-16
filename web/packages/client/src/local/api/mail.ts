import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const mailGroup: NonNullable<HttpApi["mail"]> = {
  providers: async () => ({ composio: false, imap: false, presets: [] }),
  accounts: async () => [],
  connectImap: notAvailable,
  connectComposio: notAvailable,
  update: notAvailable,
  suggestedAddresses: async () => [],
  disconnect: notAvailable,
  folders: async () => [],
  agent: async () => [],
  setAgent: notAvailable,
};
