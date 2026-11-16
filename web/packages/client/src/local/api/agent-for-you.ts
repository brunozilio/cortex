import type { HttpApi } from "../../api/client";

export const agentForYouGroup: NonNullable<HttpApi["agentForYou"]> = {
  get: async () => ({ items: [], monthCount: 0 }),
};
