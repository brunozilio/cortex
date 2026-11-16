import { describe, expect, test } from "bun:test";
import type { FetchLike } from "../src/api/http";
import { ApiError } from "../src/api/http";
import { landingFor, membershipIn, settingUp } from "../src/lib/guards";
import { can } from "../src/lib/permissions";
import { APP_API } from "../src/lib/server";
import { createLocalApi, INSTANCE_PATH, instanceOf, LOCAL_ORG_ID } from "../src/local";

function fakeFetch(routes: Record<string, { status: number; body: unknown }>) {
  const calls: { url: string; init: RequestInit }[] = [];
  const fetch: FetchLike = async (url, init) => {
    calls.push({ url, init });
    const route = routes[url];
    if (!route) return new Response("{}", { status: 404 });
    return new Response(JSON.stringify(route.body), { status: route.status });
  };
  return { fetch, calls };
}

const named = (name: string) =>
  fakeFetch({
    [INSTANCE_PATH]: { status: 200, body: { name, version: "1.2.3" } },
  });

async function rejection(run: () => Promise<unknown>): Promise<ApiError> {
  try {
    await run();
  } catch (error) {
    if (error instanceof ApiError) return error;
    throw error;
  }
  throw new Error("expected a rejection");
}

describe("local api: instance", () => {
  test("system.instance reads GET /api/instance on the same origin", async () => {
    const { fetch, calls } = named("Acme Agent");
    const instance = await createLocalApi({ fetch }).system.instance();
    expect(calls[0]?.url).toBe("/api/instance");
    expect(calls[0]?.init.method).toBe("GET");
    expect(instance.name).toBe("Acme Agent");
    expect(instance.version).toBe("1.2.3");
  });

  test("fields the server leaves out get local defaults", () => {
    const instance = instanceOf({ name: "N", version: "v" }, "http://127.0.0.1:7000");
    expect(instance).toEqual({
      name: "N",
      version: "v",
      api: APP_API,
      minApi: APP_API,
      url: "http://127.0.0.1:7000",
      signIn: { email: false, google: false, github: false, apple: false },
      signup: "invite",
      ownerClaim: false,
      cloud: false,
    });
  });

  test("fields the server sends win over the defaults", () => {
    const instance = instanceOf({
      name: "N",
      version: "v",
      api: 3,
      url: "https://x",
      signup: "open",
      cloud: true,
    });
    expect(instance.api).toBe(3);
    expect(instance.url).toBe("https://x");
    expect(instance.signup).toBe("open");
    expect(instance.cloud).toBe(true);
  });

  test("a failed instance read is an ApiError with the server's status", async () => {
    const { fetch } = fakeFetch({
      [INSTANCE_PATH]: { status: 503, body: { error: { code: "draining" } } },
    });
    const error = await rejection(() => createLocalApi({ fetch }).system.instance());
    expect(error.status).toBe(503);
    expect(error.code).toBe("draining");
  });

  test("the instance is asked once and shared", async () => {
    const { fetch, calls } = named("A");
    const api = createLocalApi({ fetch });
    await Promise.all([api.system.instance(), api.account.me(), api.account.deviceAccounts()]);
    expect(calls.filter((c) => c.url === INSTANCE_PATH)).toHaveLength(1);
  });

  test("health reads GET /api/health and says degraded when it fails", async () => {
    const ok = fakeFetch({
      "/api/health": { status: 200, body: { status: "ok", version: "9" } },
    });
    expect(await createLocalApi({ fetch: ok.fetch }).system.health()).toEqual({
      status: "ok",
      version: "9",
      edition: "selfhost",
    });
    const down = fakeFetch({});
    expect((await createLocalApi({ fetch: down.fetch }).system.health()).status).toBe("degraded");
  });
});

describe("local api: the stub person and workspace", () => {
  test("me is the local user, named after the instance, owner of the local workspace", async () => {
    const me = await createLocalApi(named("Acme Agent")).account.me();
    expect(me?.user.id).toBe("local");
    expect(me?.user.name).toBe("Acme Agent");
    expect(me?.organizations).toHaveLength(1);
    const org = me?.organizations[0];
    expect(org?.id).toBe(LOCAL_ORG_ID);
    expect(org?.name).toBe("Local");
    expect(org?.role).toBe("owner");
  });

  test("without an instance the user is still there", async () => {
    const me = await createLocalApi(fakeFetch({})).account.me();
    expect(me?.user.name).toBe("You");
  });

  test("the guards land straight on the local workspace with every permission", async () => {
    const me = await createLocalApi(named("A")).account.me();
    if (!me) throw new Error("no me");
    expect(landingFor(me, null)).toEqual({ orgId: "local" });
    const membership = membershipIn(me, "local");
    expect(membership && settingUp(membership)).toBe(false);
    expect(can(membership, "create_agents")).toBe(true);
    expect(can(membership, "manage_owners")).toBe(true);
    expect(membership?.standing?.state).toBe("active");
  });

  test("the workspace reads answer for the local org and 404 for any other", async () => {
    const api = createLocalApi(named("A"));
    expect((await api.orgs.get("local")).id).toBe("local");
    expect((await api.orgs.settings("local")).name).toBe("Local");
    expect(await api.orgs.members("local")).toHaveLength(1);
    expect((await rejection(() => api.orgs.get("other"))).status).toBe(404);
  });

  test("auth config has every sign-in off and no error reports", async () => {
    const config = await createLocalApi(named("A")).account.authConfig();
    expect(config.signupOpen).toBe(false);
    expect(config.socialProviders).toEqual({
      google: false,
      github: false,
      apple: false,
    });
    expect(config.errorReports).toBeNull();
  });
});

describe("local api: everything else", () => {
  const api = createLocalApi(named("A"));

  test("lists are empty", async () => {
    expect(await api.chat.channels("local")).toEqual([]);
    expect(await api.agents.list("local")).toEqual([]);
    expect(await api.approvals.list("local")).toEqual([]);
    expect(await api.wiki.tree("local")).toEqual([]);
  });

  test("reads the original nulls on 404 are null", async () => {
    expect(await api.system.telemetry()).toBeNull();
    expect(await api.push.webKey()).toBeNull();
    expect(await api.orgs.previewInvitation("t")).toBeNull();
  });

  test("a read of one entity is a 404", async () => {
    expect((await rejection(() => api.agents.detail("local", "a1"))).status).toBe(404);
  });

  test("writes are 501 not_available", async () => {
    for (const run of [
      () => api.chat.createChannel("local", { name: "x" } as never),
      () => api.orgs.update("local", { name: "x" }),
      () => api.account.updateMe({ locale: "en" }),
      () => api.dashboards.create("local", {} as never),
    ]) {
      const error = await rejection(run);
      expect(error.status).toBe(501);
      expect(error.code).toBe("not_available");
    }
  });
});
