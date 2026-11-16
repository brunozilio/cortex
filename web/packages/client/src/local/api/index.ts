import type { HttpApi } from "../../api/client";
import { accessGroup } from "./access";
import { accountGroup } from "./account";
import { actionCardsGroup } from "./action-cards";
import { actionsGroup } from "./actions";
import { agentForYouGroup } from "./agent-for-you";
import { agentGroupsGroup } from "./agent-groups";
import { agentsGroup } from "./agents";
import { approvalsGroup } from "./approvals";
import { appsGroup } from "./apps";
import { artifactsGroup } from "./artifacts";
import { browserGroup } from "./browser";
import { browserProfilesGroup } from "./browser-profiles";
import { calendarGroup } from "./calendar";
import { chatGroup } from "./chat";
import { connectorsGroup } from "./connectors";
import { customConnectorsGroup } from "./custom-connectors";
import { dashboardsGroup } from "./dashboards";
import { databasesGroup } from "./databases";
import { desktopGroup } from "./desktop";
import { devicesGroup } from "./devices";
import { dmsGroup } from "./dms";
import { gifsGroup } from "./gifs";
import { githubAppGroup } from "./github-app";
import { importsGroup } from "./imports";
import { invitesGroup } from "./invites";
import { issuesGroup } from "./issues";
import { joinRequestsGroup } from "./join-requests";
import { machinesGroup } from "./machines";
import { mailGroup } from "./mail";
import { memoryGroup } from "./memory";
import { missionsGroup } from "./missions";
import { notificationsGroup } from "./notifications";
import { orgsGroup } from "./orgs";
import { pinsGroup } from "./pins";
import { proposalsGroup } from "./proposals";
import { pushGroup } from "./push";
import { questionsGroup } from "./questions";
import { readsGroup } from "./reads";
import { remindersGroup } from "./reminders";
import { repositoriesGroup } from "./repositories";
import { rolesGroup } from "./roles";
import { routinesGroup } from "./routines";
import { sandboxGroup } from "./sandbox";
import { scopesGroup } from "./scopes";
import { searchGroup } from "./search";
import { secretsGroup } from "./secrets";
import { skillsGroup } from "./skills";
import { storageGroup } from "./storage";
import { systemGroup } from "./system";
import { threadFollowsGroup } from "./thread-follows";
import { threadsGroup } from "./threads";
import { vaultGroup } from "./vault";
import { voiceGroup } from "./voice";
import { wikiGroup } from "./wiki";
import { workingGroup } from "./working";

export const generatedLocalApi: HttpApi = {
  system: systemGroup,
  voice: voiceGroup,
  account: accountGroup,
  push: pushGroup,
  orgs: orgsGroup,
  roles: rolesGroup,
  agentGroups: agentGroupsGroup,
  agents: agentsGroup,
  imports: importsGroup,
  chat: chatGroup,
  approvals: approvalsGroup,
  connectors: connectorsGroup,
  githubApp: githubAppGroup,
  search: searchGroup,
  sandbox: sandboxGroup,
  devices: devicesGroup,
  machines: machinesGroup,
  skills: skillsGroup,
  access: accessGroup,
  repositories: repositoriesGroup,
  scopes: scopesGroup,
  memory: memoryGroup,
  proposals: proposalsGroup,
  customConnectors: customConnectorsGroup,
  dms: dmsGroup,
  questions: questionsGroup,
  secrets: secretsGroup,
  issues: issuesGroup,
  threads: threadsGroup,
  dashboards: dashboardsGroup,
  missions: missionsGroup,
  routines: routinesGroup,
  wiki: wikiGroup,
  notifications: notificationsGroup,
  calendar: calendarGroup,
  artifacts: artifactsGroup,
  storage: storageGroup,
  invites: invitesGroup,
  joinRequests: joinRequestsGroup,
  pins: pinsGroup,
  actions: actionsGroup,
  actionCards: actionCardsGroup,
  agentForYou: agentForYouGroup,
  threadFollows: threadFollowsGroup,
  reads: readsGroup,
  working: workingGroup,
  mail: mailGroup,
  databases: databasesGroup,
  browser: browserGroup,
  browserProfiles: browserProfilesGroup,
  vault: vaultGroup,
  desktop: desktopGroup,
  gifs: gifsGroup,
  apps: appsGroup,
  reminders: remindersGroup,
};
