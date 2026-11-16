import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const customConnectorsGroup: NonNullable<HttpApi["customConnectors"]> = {
  list: async () => [],
  connect: notAvailable,
  refresh: notAvailable,
  saveTools: notAvailable,
  disconnect: notAvailable,
};
