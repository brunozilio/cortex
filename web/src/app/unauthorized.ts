import type { QueryClient } from "@tanstack/react-query";
import type { i18n as I18n } from "i18next";
import type { ToastStore } from "@/lib/toast";

export function sessionEnded(app: { queryClient: QueryClient; toasts: ToastStore; i18n: I18n }) {
  app.queryClient.clear();
  app.toasts.show(app.i18n.t("common.signedOut"));
}
