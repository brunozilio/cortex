import type { HttpApi } from "../../api/client";
import { notAvailable } from "../unavailable";

export const actionsGroup: NonNullable<HttpApi["actions"]> = {
  undo: notAvailable,
  redo: notAvailable,
  undoMessage: notAvailable,
};
