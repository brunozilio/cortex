import { Button, EmptyState } from "@agenteschat/ui";
import { createRootRouteWithContext, Link, Outlet } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import type { RouterContext } from "@/app/router-context";
import { ContextMenuHost } from "@/components/ContextMenus";
import { ErrorScreen } from "@/components/ErrorScreen";
import { useBackCloses } from "@/hooks/useBackCloses";

export const Route = createRootRouteWithContext<RouterContext>()({
  component: Root,
  notFoundComponent: NotFound,
  errorComponent: ErrorScreen,
});

function Root() {
  useBackCloses();
  return (
    <>
      <ContextMenuHost />
      <Outlet />
    </>
  );
}

function NotFound() {
  const { t } = useTranslation();
  return (
    <div className="page page--mid">
      <header className="page__header">
        <h1 className="page-title">{t("empty.notFoundTitle")}</h1>
      </header>
      <EmptyState
        icon="search"
        actions={
          <Button asChild>
            <Link to="/">{t("empty.goHome")}</Link>
          </Button>
        }
      >
        {t("empty.notFoundText")}
      </EmptyState>
    </div>
  );
}
