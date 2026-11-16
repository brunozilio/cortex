import { meQuery } from "@agenteschat/client";
import { landingFor } from "@agenteschat/client/lib/guards";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { NoSession } from "@/app/guards";
import { showWorkspace } from "@/lib/start";
import { STORAGE_KEYS, storage } from "@/lib/storage";

export const Route = createFileRoute("/")({
  beforeLoad: async ({ context, preload }) => {
    const me = await context.queryClient.ensureQueryData(meQuery(context.api));
    if (!me) throw new NoSession();
    const landing = landingFor(me, storage.get(STORAGE_KEYS.lastOrg));
    const orgId = "orgId" in landing ? landing.orgId : me.organizations[0]?.id;
    if (!orgId) throw new NoSession();
    if (!preload) showWorkspace(null);
    throw redirect({ to: "/org/$orgId", params: { orgId } });
  },
});
