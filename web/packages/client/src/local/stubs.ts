import type {
  AuthConfig,
  DeviceAccount,
  Me,
  Member,
  Organization,
  OrgOverview,
  OrgSettings,
  OrgSummary,
  Person,
} from "../api/types";
import type { Permission } from "../lib/permissions";
import { SUMMARY_ORDER } from "../lib/roles";

export const LOCAL_ORG_ID = "local";
export const LOCAL_USER_ID = "local";
export const LOCAL_ORG_NAME = "Local";
export const LOCAL_USER_FALLBACK_NAME = "You";
export const LOCAL_JOINED_AT = "1970-01-01T00:00:00.000Z";

export const OWNER_PERMISSIONS: Permission[] = ["read", "manage_owners", ...SUMMARY_ORDER];

const timezone = () => {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  } catch {
    return "UTC";
  }
};

export const localUserName = (instanceName: string | null | undefined) =>
  instanceName?.trim() || LOCAL_USER_FALLBACK_NAME;

export function localOrgSummary(): OrgSummary {
  return {
    id: LOCAL_ORG_ID,
    name: LOCAL_ORG_NAME,
    role: "owner",
    permissions: [...OWNER_PERMISSIONS],
    rank: 0,
    logo: null,
    createdAt: LOCAL_JOINED_AT,
    standing: { state: "active", reason: null },
    start: { page: "home", channelId: null },
    showActivity: false,
    agentStyle: null,
    mode: "team",
  };
}

export function localMe(name: string): Me {
  return {
    user: {
      id: LOCAL_USER_ID,
      name,
      email: "",
      image: null,
      hasPassword: false,
      emailVerified: true,
      canCreateWorkspace: false,
      locale: null,
      localeAuto: true,
      workArea: null,
    },
    organizations: [localOrgSummary()],
  };
}

export function localOrganization(): Organization {
  return { ...localOrgSummary(), context: "" };
}

export function localMember(name: string): Member {
  return {
    userId: LOCAL_USER_ID,
    name,
    email: "",
    image: null,
    avatarUrl: null,
    role: "owner",
    roleIds: [],
    joinedAt: LOCAL_JOINED_AT,
    handle: LOCAL_USER_ID,
    timezone: timezone(),
  };
}

export function localPerson(name: string): Person {
  return {
    ...localMember(name),
    lastSeenAt: null,
    online: true,
    presence: "online",
    status: null,
    availability: "auto",
  };
}

export function localDeviceAccount(name: string): DeviceAccount {
  const me = localMe(name);
  return {
    current: true,
    user: {
      id: me.user.id,
      name: me.user.name,
      email: me.user.email,
      image: me.user.image,
    },
    organizations: me.organizations,
    unread: [{ orgId: LOCAL_ORG_ID, unread: 0, waiting: 0 }],
  };
}

export function localAuthConfig(): AuthConfig {
  return {
    socialProviders: { google: false, github: false, apple: false },
    signupOpen: false,
    firstRun: false,
    ownerClaim: false,
    passwordReset: false,
    termsUrl: null,
    privacyUrl: null,
    supportEmail: null,
    feedback: false,
    feedbackRelay: false,
    workspaceComputer: false,
    billingUrl: null,
    cloud: false,
    turnstileSiteKey: null,
    errorReports: null,
    mobileErrorReports: null,
  };
}

export function localOrgSettings(): OrgSettings {
  return {
    name: LOCAL_ORG_NAME,
    context: "",
    defaultCredentialId: null,
    routerModel: null,
    summaryModel: null,
    timezone: timezone(),
    actionCheck: true,
    autoModels: {},
    gifs: false,
    gifRating: "g",
    agentStickers: false,
    agentMedia: "never",
    voiceMessages: false,
    teamWatch: false,
    welcomePeople: false,
    voiceAvailable: false,
    gifsAvailable: false,
    agentMediaAvailable: false,
    mode: "team",
    approvalsLevel: "risky",
  };
}

export function localOrgOverview(): OrgOverview {
  return {
    name: LOCAL_ORG_NAME,
    role: "owner",
    people: 1,
    agents: 0,
    budgetUsedPercent: 0,
    approvalsWaitingForMe: 0,
    proposalsWaitingForMe: 0,
  };
}
