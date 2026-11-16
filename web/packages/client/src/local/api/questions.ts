import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const questionsGroup: NonNullable<HttpApi["questions"]> = {
  answer: notAvailable,
  dismiss: notAvailable,
};
