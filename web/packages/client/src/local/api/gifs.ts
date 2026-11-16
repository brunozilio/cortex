import type { HttpApi } from "../../api/client";

export const gifsGroup: NonNullable<HttpApi["gifs"]> = {
  search: async () => ({ gifs: [], next: null }),
};
