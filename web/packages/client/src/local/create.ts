import type { HttpApi } from "../api/client";
import type { FetchLike } from "../api/http";
import type { Instance } from "../api/types";
import { ISSUES_VIEW_DEFAULT } from "../lib/issues-view";
import { sidebarItemOrder, sidebarSections } from "../lib/sidebar-items";
import { generatedLocalApi } from "./api";
import { fetchHealth, fetchInstance, localTransport } from "./instance";
import {
  LOCAL_ORG_ID,
  localAuthConfig,
  localDeviceAccount,
  localMe,
  localMember,
  localOrganization,
  localOrgOverview,
  localOrgSettings,
  localPerson,
  localUserName,
} from "./stubs";
import { notFound } from "./unavailable";

export interface LocalApiOptions {
  fetch?: FetchLike;
  baseUrl?: string;
}

export function createLocalApi(options: LocalApiOptions = {}): HttpApi {
  const transport = localTransport(options);
  let instance: Promise<Instance> | null = null;
  const loadInstance = () => {
    const pending = instance ?? fetchInstance(transport);
    instance = pending;
    pending.catch(() => {
      if (instance === pending) instance = null;
    });
    return pending;
  };
  const userName = () =>
    loadInstance().then(
      (found) => localUserName(found.name),
      () => localUserName(null),
    );
  const base = generatedLocalApi;
  const isLocal = (orgId: string) => orgId === LOCAL_ORG_ID;

  return {
    ...base,
    system: {
      ...base.system,
      instance: loadInstance,
      health: () => fetchHealth(transport),
    },
    account: {
      ...base.account,
      authConfig: async () => localAuthConfig(),
      me: async () => localMe(await userName()),
      unread: async () => ({
        organizations: [{ orgId: LOCAL_ORG_ID, unread: 0, waiting: 0 }],
      }),
      deviceAccounts: async () => [localDeviceAccount(await userName())],
    },
    orgs: {
      ...base.orgs,
      get: async (orgId) => (isLocal(orgId) ? localOrganization() : notFound()),
      settings: async (orgId) => (isLocal(orgId) ? localOrgSettings() : notFound()),
      overview: async (orgId) => (isLocal(orgId) ? localOrgOverview() : notFound()),
      members: async (orgId) => (isLocal(orgId) ? [localMember(await userName())] : []),
      people: async (orgId) => (isLocal(orgId) ? [localPerson(await userName())] : []),
      issuesView: async () => structuredClone(ISSUES_VIEW_DEFAULT),
      sidebarOrder: async () => [...sidebarSections(false)],
      sidebarToolOrder: async () => [...sidebarItemOrder(false)],
    },
  };
}
