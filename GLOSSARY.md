# Superfik

A canvas where the people of a technology company work with AI agents,
alone or together.

## People and organizations

**Organization**:
The team (or the single person, in local mode) a server serves, and
everything its members create.
_Avoid_: Tenant, account, company, workspace

**Member**:
A person who belongs to an organization and signs in to its server with
the username and password they chose on their first access.
_Avoid_: Seat, collaborator

**First-access code**:
The code (`XXXX-XXXX-XXXX`, once, for 7 days) an owner or admin hands a new
member, or one who forgot their password, so they can choose a password.
_Avoid_: Invitation, token, OTP

**Team**:
A named group of members inside an organization.
_Avoid_: Group, squad

**Role**:
What a member may do, in the organization (Owner, Admin, Member, Guest) or
in a project (Maintainer, Contributor, Viewer).
_Avoid_: Permission (that word belongs to agent asks)

**Principal**:
Who is acting on a command: the local owner or an authenticated member.

## Work

**Project**:
What a set of sessions is about, identified by its git remote, or a space
with no repository.
_Avoid_: Workspace (for the shared thing), repo

**Session**:
One conversation with agents, with its turns, context and canvas.
_Avoid_: Conversation (in product language), chat, thread

**Visibility**:
Who can see a session: private, shared with members or teams, or the whole
project.

**Scope**:
Where a memory lives and who it reaches: personal, project, team or
organization.

## Running

**Run**:
One execution of an engine for a prompt or a session command, ending in
exactly one terminal event.

**Runner**:
Where a run executes: always the server the client is connected to
(ADR 0010), as the OS user `superfik-runner`.
_Avoid_: Worker, agent host

**Client**:
The desktop or web app. It shows and asks; it never runs anything.

**Server**:
The Superfik server of an organization, embedded in the desktop for local
mode or hosted by the company.
_Avoid_: Backend (that is the client's seam to it)

## Operating

**Routine**:
A run started without a human prompt, by a schedule or a trigger, with its
own budget and autonomy.
_Avoid_: Job, automation, cron

**Channel**:
A place where the product talks to people outside the organization
(WhatsApp, e-mail, Instagram, X, LinkedIn), provided by a plugin.
_Avoid_: Integration (that is any plugin)

**Contact**:
A person outside the organization reached through one or more channels.
_Avoid_: Customer, lead, user

**Exchange**:
The messages between a project and one contact on one channel.
_Avoid_: Thread, conversation, ticket

**Autonomy**:
How far a routine may act on its own: Draft (a member approves), Bounded
(inside rules) or Auto.

**Operations inbox**:
Where members see what routines did and approve what waits for them.
_Avoid_: Queue, feed

## Running a server

**Server**:
A team's own `superfik-server`, installed with `scripts/install-server.sh`
or Docker; the app connects to it from Settings › Server. Alone, the app
runs one embedded in itself.
_Avoid_: Cloud, backend (in product language)

**Domain**:
Where members reach a server (`superfik.acme.io`), set by its owners and
admins.

**App lock**:
Touch ID as the app opens, a choice in Settings › General.
_Avoid_: Sign-in (there is none for the app itself)
