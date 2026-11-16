import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const threadFollowsGroup: NonNullable<HttpApi["threadFollows"]> = {
  follow: notAvailable,
  unfollow: notAvailable,
};
