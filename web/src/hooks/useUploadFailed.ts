import { errorText } from "@agenteschat/client/lib/errors";
import { useTranslation } from "react-i18next";
import { useToast } from "@/lib/toast";

export function useUploadFailed() {
  const { t, i18n } = useTranslation();
  const toast = useToast();
  return (error: unknown) => toast.show(errorText(t, error, i18n.language), "error");
}
