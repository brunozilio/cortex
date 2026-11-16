import type { HttpApi } from "../../api/client";
import { notAvailable, notFound } from "../unavailable";

export const dashboardsGroup: NonNullable<HttpApi["dashboards"]> = {
  list: async () => ({ home: null, dashboards: [], homeDashboardId: null }),
  get: notFound,
  home: notFound,
  setHome: notAvailable,
  create: notAvailable,
  update: notAvailable,
  share: notAvailable,
  remove: notAvailable,
  saveLayout: notAvailable,
  decideProposal: notAvailable,
  createWidget: notAvailable,
  testWidget: notAvailable,
  confirmWidget: notAvailable,
  pinWidget: notAvailable,
  widget: notFound,
  updateWidget: notAvailable,
  removeWidget: notAvailable,
  refreshWidget: notAvailable,
  askWidgetChange: notAvailable,
  widgetCost: notFound,
  widgetSuggestions: notFound,
  pinnableBlocks: async () => [],
};
