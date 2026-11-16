import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const systemGroup: NonNullable<HttpApi["system"]> = {
  health: notFound,
  instance: notFound,
  telemetry: async () => null,
  setTelemetry: notAvailable,
  sendFeedback: notAvailable,
  uploadFeedbackFile: notAvailable,
  dropFeedbackFile: notAvailable,
  decideFeedbackOffer: notAvailable,
  jev: async () => null,
  saveJev: notAvailable,
  checkJev: notAvailable,
  removeJev: notAvailable,
  gifKey: async () => null,
  saveGifKey: notAvailable,
  removeGifKey: notAvailable,
  speechKey: async () => null,
  saveSpeechKey: notAvailable,
  removeSpeechKey: notAvailable,
};
