//! Popovers: the model picker, branch list, repository list and skill picker.

use superfik_core::{Availability, BranchRef, PullRequestState, ReviewDecision};
use superfik_i18n::{Strings, Text};
use superfik_motion::Presence;
use superfik_render::{GooShape, Point, Rect};
use superfik_ui::components::{BrowserCard, BrowserPart, EarSide, Menu, MenuItem, NodeFrame, Popover, Side};
use superfik_ui::theme::colors::Palette;
use superfik_ui::theme::metrics;
use superfik_ui::{Painter, Plane, icons};

use super::layout::NodeKey;
use super::motions::Motions;
use super::nodes::skills;
use super::target::{HitMap, Target};
use crate::state::{AccountChoice, ModelRow, PopoverKind, Session, account_label};

const GAP: f32 = 14.0;
const MAX_ITEMS: usize = 6;
const REPOSITORY_PANEL: f32 = 240.0;
const EAR_CLEARANCE: f32 = 6.0;
const PERMISSION_PANEL: f32 = 200.0;
const ACCOUNT_PANEL: f32 = 200.0;
const LOGIN_PANEL: f32 = 240.0;

fn anchored_to(kind: PopoverKind) -> Option<NodeKey> {
    match kind {
        PopoverKind::Branch | PopoverKind::Repository | PopoverKind::PullRequests | PopoverKind::AttachRepository => {
            Some(NodeKey::Repository)
        }
        PopoverKind::AddSkill => Some(NodeKey::Skills),
        PopoverKind::PluginAccounts => Some(NodeKey::Connections),
        PopoverKind::Logins | PopoverKind::SaveLogin | PopoverKind::Passkeys => Some(NodeKey::Browser),
        PopoverKind::Model | PopoverKind::Skills | PopoverKind::Permission => None,
    }
}

struct Labels {
    no_pull_requests: String,
    all_skills_added: String,
    draft: String,
    approved: String,
    changes_requested: String,
    review_required: String,
    open_repository: String,
    open_pull_request: String,
    choose_repository: String,
    sign_in: String,
    sign_in_here: String,
    private: String,
    not_installed: String,
    soon: String,
    account_not_used: String,
    add_account: String,
    not_now: String,
    passkey_elsewhere: String,
    no_models: String,
}

impl Labels {
    fn new(s: &Strings) -> Self {
        Self {
            no_pull_requests: s.t(Text::NoPullRequests),
            all_skills_added: s.t(Text::AllSkillsAdded),
            draft: s.t(Text::Draft),
            approved: s.t(Text::ReviewApproved),
            changes_requested: s.t(Text::ReviewChangesRequested),
            review_required: s.t(Text::ReviewRequired),
            open_repository: s.t(Text::OpenRepositoryMenu),
            open_pull_request: s.t(Text::OpenPullRequestMenu),
            choose_repository: s.t(Text::ChooseRepositoryMenu),
            sign_in: s.t(Text::GitHubSignIn),
            sign_in_here: s.t(Text::GitHubSignInHere),
            private: s.t(Text::Private),
            not_installed: s.t(Text::NotInstalled),
            soon: s.t(Text::Soon),
            account_not_used: s.t(Text::AccountNotUsed),
            add_account: s.t(Text::AddAccount),
            not_now: s.t(Text::NotNow),
            passkey_elsewhere: s.t(Text::PasskeyElsewhere),
            no_models: s.t(Text::NoModels),
        }
    }
}

pub fn workspace_label(path: &std::path::Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

fn github_item<'a>(c: &Palette, slug: &'a str) -> MenuItem<'a> {
    MenuItem { icon: Some((icons::REPOSITORY, Some(c.repository))), ..MenuItem::new(slug) }
}

fn sign_in_row<'a>(session: &Session, labels: &'a Labels) -> Option<(MenuItem<'a>, Target)> {
    let answered = session.github.is_some();
    if !answered || session.github_signed_in() {
        return None;
    }
    Some(if session.signs_in_to_github() {
        (MenuItem::new(&labels.sign_in_here), Target::SignInToGitHub)
    } else {
        (MenuItem { muted: true, ..MenuItem::new(&labels.sign_in) }, Target::Surface)
    })
}

pub fn branches(session: &Session) -> Vec<BranchRef> {
    let mut all = session.snapshot.repository.as_ref().map(|r| r.branch_choices()).unwrap_or_default();
    all.truncate(MAX_ITEMS);
    all
}

fn model_row<'a>(
    s: &'a Session,
    row: ModelRow,
    title: &'a str,
    labels: &'a Labels,
    c: &Palette,
) -> Option<(MenuItem<'a>, Target)> {
    match row {
        ModelRow::Account { .. } => Some((MenuItem { muted: true, ..MenuItem::new(title) }, Target::Surface)),
        ModelRow::Engine { kind, availability } => {
            let badge = match availability {
                Availability::Ready => None,
                Availability::NotInstalled => Some(labels.not_installed.as_str()),
                Availability::Soon { .. } => Some(labels.soon.as_str()),
            };
            Some((MenuItem { badge, muted: true, ..MenuItem::new(kind.label()) }, Target::Surface))
        }
        ModelRow::Model(i) => {
            let model = s.models.get(i)?;
            let item = MenuItem {
                icon: Some((icons::BURST, Some(c.model))),
                selected: i == s.conversation().model,
                ..MenuItem::new(&model.label)
            };
            Some((item, Target::ModelItem(i)))
        }
    }
}

fn below_chip(composer: Rect, chip: Rect, h: f32) -> Placement {
    let shape = GooShape::RoundRect { rect: composer, radius: metrics::NODE_RADIUS };
    Placement {
        anchor: Point::new(chip.center().x, composer.bottom()),
        side: Side::Down,
        panel: Rect::new(chip.x, composer.bottom() + GAP, PERMISSION_PANEL.max(chip.w), h),
        source: shape,
    }
}

pub struct PopoverView<'a> {
    pub session: &'a Session,
    pub motions: &'a Motions,
    pub composer: Rect,
    pub permission_chip: Option<Rect>,
    pub hovered: Option<Target>,
    pub now: f64,
}

struct RowLabels {
    branch: Vec<String>,
    repo: Vec<String>,
    attach: Vec<String>,
    pull_request: Vec<String>,
    permission: Vec<String>,
    add_skill: Vec<String>,
    pick: Vec<String>,
    login: Vec<String>,
    offer: String,
    save: Vec<String>,
    passkey: Vec<String>,
    model: Vec<String>,
}

struct Placement {
    anchor: Point,
    side: Side,
    panel: Rect,
    source: GooShape,
}

impl PopoverView<'_> {
    pub fn paint(&self, p: &mut Painter, hits: &mut HitMap) {
        let open: Vec<(PopoverKind, Presence)> = self.motions.popovers().map(|(k, v)| (*k, *v)).collect();
        let rows = RowLabels {
            branch: branches(self.session).iter().map(BranchRef::label).collect(),
            repo: self.session.recent_choices().iter().take(MAX_ITEMS).map(|p| workspace_label(p)).collect(),
            attach: self.session.attachable_repositories().iter().take(MAX_ITEMS).map(|p| workspace_label(p)).collect(),
            pull_request: self
                .session
                .pull_requests
                .iter()
                .take(MAX_ITEMS)
                .map(|pr| format!("#{} {}", pr.number, pr.title))
                .collect(),
            permission: superfik_core::PermissionMode::ALL
                .iter()
                .map(|mode| super::ask::mode_label(&self.session.strings(), *mode))
                .collect(),
            add_skill: self.session.addable_skills().iter().map(|s| s.title.clone()).collect(),
            pick: self.session.picks().iter().map(|pick| pick.label().trim_start_matches('/').to_owned()).collect(),
            login: self.session.browser().map(|b| b.logins()).unwrap_or_default().iter().map(login_label).collect(),
            offer: self.session.browser().and_then(|b| b.offer.as_ref()).map(offer_label).unwrap_or_default(),
            passkey: passkey_rows(self.session),
            model: self
                .session
                .model_rows()
                .into_iter()
                .map(|row| match row {
                    ModelRow::Account { provider, account } => self.session.account_title(provider, account),
                    _ => String::new(),
                })
                .collect(),
            save: {
                let strings = self.session.strings();
                self.session.vault_accounts().iter().map(|a| strings.t(Text::SaveLoginTo(a))).collect()
            },
        };
        let labels = Labels::new(&self.session.strings());
        for (kind, presence) in open {
            let progress = presence.progress(self.now);
            if kind == PopoverKind::Skills {
                continue;
            }
            let (items, targets) = self.items(kind, &rows, &labels, p.palette());
            if items.is_empty() {
                continue;
            }
            let Some(place) = self.place(kind, items.len()) else { continue };
            if presence.is_visible() {
                hits.attached.push((anchored_to(kind), place.panel));
            }
            p.on_plane(Plane::Overlay, |p| {
                let frame = Popover {
                    anchor: place.anchor,
                    side: place.side,
                    panel: place.panel,
                    progress,
                    source: Some(place.source),
                    closing: !presence.is_visible(),
                }
                .paint(p);
                let hovered = targets.iter().position(|t| *t != Target::Surface && Some(*t) == self.hovered);
                let rows = p.faded(frame.content_opacity, |p| {
                    p.blurred(frame.content_blur, |p| {
                        if kind == PopoverKind::Model {
                            let max = (Menu::height(items.len()) - frame.panel.h).max(0.0);
                            let offset = self.session.model_scroll.clamp(0.0, max);
                            let content = Rect::new(
                                frame.panel.x,
                                frame.panel.y - offset,
                                frame.panel.w,
                                Menu::height(items.len()),
                            );
                            p.clipped(frame.panel, |p| Menu { items: &items, hovered }.paint(p, content))
                        } else {
                            Menu { items: &items, hovered }.paint(p, frame.panel)
                        }
                    })
                });
                if presence.is_visible() && progress > 0.6 {
                    hits.add(frame.panel, Target::Surface);
                    if kind == PopoverKind::Model {
                        hits.model_scroll = Some((frame.panel, (Menu::height(items.len()) - frame.panel.h).max(0.0)));
                    }
                    for (rect, target) in rows.into_iter().zip(targets) {
                        let rect = if kind == PopoverKind::Model { rect.intersect(&frame.panel) } else { rect };
                        if rect.w > 0.0 && rect.h > 0.0 {
                            hits.add(rect, target);
                        }
                    }
                }
            });
        }
    }

    fn items<'a>(
        &'a self,
        kind: PopoverKind,
        rows: &'a RowLabels,
        labels: &'a Labels,
        c: &Palette,
    ) -> (Vec<MenuItem<'a>>, Vec<Target>) {
        let s = self.session;
        match kind {
            PopoverKind::Permission => superfik_core::PermissionMode::ALL
                .iter()
                .zip(&rows.permission)
                .map(|(mode, label)| {
                    let item = MenuItem {
                        icon: Some((icons::SHIELD, None)),
                        selected: *mode == s.conversation().permission,
                        ..MenuItem::new(label)
                    };
                    (item, Target::PermissionItem(*mode))
                })
                .unzip(),
            PopoverKind::Model if s.model_rows().is_empty() => {
                (vec![MenuItem { muted: true, ..MenuItem::new(&labels.no_models) }], vec![Target::Surface])
            }
            PopoverKind::Model => s
                .model_rows()
                .into_iter()
                .zip(&rows.model)
                .filter_map(|(row, title)| model_row(s, row, title, labels, c))
                .unzip(),
            PopoverKind::Branch => {
                let current = s.branch.as_deref().unwrap_or_default();
                branches(s)
                    .iter()
                    .zip(&rows.branch)
                    .enumerate()
                    .map(|(i, (b, label))| {
                        (
                            MenuItem {
                                icon: Some((icons::BRANCH, Some(c.branch))),
                                selected: !b.remote && b.name == current,
                                ..MenuItem::new(label)
                            },
                            Target::BranchItem(i),
                        )
                    })
                    .unzip()
            }
            PopoverKind::Repository => {
                let current = s.snapshot.repository.as_ref().map(|r| r.slug.as_str());
                let github = s.github_choices().into_iter().enumerate().map(|(i, repo)| {
                    (
                        MenuItem {
                            selected: current.is_some_and(|c| c.eq_ignore_ascii_case(&repo.slug)),
                            badge: repo.private.then_some(labels.private.as_str()),
                            ..github_item(c, &repo.slug)
                        },
                        Target::GitHubRepositoryItem(i),
                    )
                });
                let recents = s.recent_choices();
                let local = rows.repo.iter().enumerate().map(|(i, label)| {
                    let current = s.root.as_deref() == recents.get(i).copied();
                    (
                        MenuItem {
                            icon: Some((icons::REPOSITORY, Some(c.repository))),
                            selected: current,
                            ..MenuItem::new(label)
                        },
                        Target::RepositoryItem(i),
                    )
                });
                sign_in_row(s, labels)
                    .into_iter()
                    .chain(github)
                    .chain(local)
                    .chain(s.opens_folders().then(|| {
                        (
                            MenuItem {
                                icon: Some((icons::PLUS_CIRCLE, None)),
                                ..MenuItem::new(&labels.open_repository)
                            },
                            Target::ChooseRepository,
                        )
                    }))
                    .unzip()
            }
            PopoverKind::AttachRepository => {
                let github = s.github_attachable().into_iter().enumerate().map(|(i, repo)| {
                    (
                        MenuItem {
                            badge: repo.private.then_some(labels.private.as_str()),
                            ..github_item(c, &repo.slug)
                        },
                        Target::AttachGitHubRepositoryItem(i),
                    )
                });
                let local = rows.attach.iter().enumerate().map(|(i, label)| {
                    (
                        MenuItem { icon: Some((icons::REPOSITORY, Some(c.repository))), ..MenuItem::new(label) },
                        Target::AttachRepositoryItem(i),
                    )
                });
                sign_in_row(s, labels)
                    .into_iter()
                    .chain(github)
                    .chain(local)
                    .chain(s.opens_folders().then(|| {
                        (
                            MenuItem {
                                icon: Some((icons::PLUS_CIRCLE, None)),
                                ..MenuItem::new(&labels.choose_repository)
                            },
                            Target::ChooseAttachedRepository,
                        )
                    }))
                    .unzip()
            }
            PopoverKind::PullRequests => {
                let open = s.can_open_pull_request().then(|| {
                    (
                        MenuItem { icon: Some((icons::PLUS_CIRCLE, None)), ..MenuItem::new(&labels.open_pull_request) },
                        Target::OpenPullRequest,
                    )
                });
                let none = rows
                    .pull_request
                    .is_empty()
                    .then(|| (MenuItem { muted: true, ..MenuItem::new(&labels.no_pull_requests) }, Target::Surface));
                let listed = s.pull_requests.iter().zip(&rows.pull_request).enumerate().map(|(i, (pr, label))| {
                    let current = s.conversation().pull_request.as_ref().is_some_and(|c| c.number == pr.number);
                    let badge = match (pr.state, pr.review_decision) {
                        (PullRequestState::Draft, _) => Some(labels.draft.as_str()),
                        (_, Some(ReviewDecision::Approved)) => Some(labels.approved.as_str()),
                        (_, Some(ReviewDecision::ChangesRequested)) => Some(labels.changes_requested.as_str()),
                        (_, Some(ReviewDecision::ReviewRequired)) => Some(labels.review_required.as_str()),
                        _ => None,
                    };
                    (
                        MenuItem {
                            icon: Some((icons::PULL_REQUEST, Some(c.branch))),
                            selected: current,
                            badge,
                            ..MenuItem::new(label)
                        },
                        Target::PullRequestItem(i),
                    )
                });
                none.into_iter().chain(listed).chain(open).unzip()
            }
            PopoverKind::AddSkill => {
                let none = rows
                    .add_skill
                    .is_empty()
                    .then(|| (MenuItem { muted: true, ..MenuItem::new(&labels.all_skills_added) }, Target::Surface));
                let listed =
                    s.addable_skills().into_iter().zip(&rows.add_skill).enumerate().map(|(i, (skill, label))| {
                        (
                            MenuItem { icon: Some(skills::icon(c, skill.icon)), ..MenuItem::new(label) },
                            Target::AddSkillItem(i),
                        )
                    });
                none.into_iter().chain(listed).unzip()
            }
            PopoverKind::PluginAccounts => {
                let glyph = s.account_menu_row().map(|row| super::nodes::connections::plugin_glyph(c, row.icon));
                s.account_choices()
                    .into_iter()
                    .enumerate()
                    .map(|(i, choice)| {
                        let item = match choice {
                            AccountChoice::Account { connection, selected } => {
                                MenuItem { icon: glyph, selected, ..MenuItem::new(account_label(connection)) }
                            }
                            AccountChoice::NotUsed { selected } => MenuItem {
                                icon: Some((icons::CLOSE, None)),
                                selected,
                                ..MenuItem::new(&labels.account_not_used)
                            },
                            AccountChoice::Add => MenuItem {
                                icon: Some((icons::PLUS_CIRCLE, None)),
                                ..MenuItem::new(&labels.add_account)
                            },
                        };
                        (item, Target::AccountChoice(i))
                    })
                    .unzip()
            }
            PopoverKind::Logins => {
                let key = |label| MenuItem { icon: Some((icons::KEY, None)), ..MenuItem::new(label) };
                let logins = rows.login.iter().enumerate().map(|(i, label)| (key(label), Target::LoginItem(i)));
                let passkeys = rows.passkey.iter().enumerate().map(|(i, label)| (key(label), Target::PasskeyItem(i)));
                logins.chain(passkeys).unzip()
            }
            PopoverKind::SaveLogin => {
                let offer = (MenuItem { muted: true, ..MenuItem::new(&rows.offer) }, Target::Surface);
                let accounts = rows.save.iter().enumerate().map(|(i, label)| {
                    (MenuItem { icon: Some((icons::KEY, None)), ..MenuItem::new(label) }, Target::SaveLoginItem(i))
                });
                let not_now = (
                    MenuItem { icon: Some((icons::CLOSE, None)), ..MenuItem::new(&labels.not_now) },
                    Target::DismissSaveLogin,
                );
                std::iter::once(offer).chain(accounts).chain(std::iter::once(not_now)).unzip()
            }
            PopoverKind::Passkeys => {
                let passkeys = rows.passkey.iter().enumerate().map(|(i, label)| {
                    (MenuItem { icon: Some((icons::KEY, None)), ..MenuItem::new(label) }, Target::PasskeyItem(i))
                });
                let elsewhere = (
                    MenuItem {
                        icon: Some((icons::CLOSE, None)),
                        muted: true,
                        ..MenuItem::new(&labels.passkey_elsewhere)
                    },
                    Target::PasskeyElsewhere,
                );
                passkeys.chain(std::iter::once(elsewhere)).unzip()
            }
            PopoverKind::Skills => s
                .picks()
                .iter()
                .enumerate()
                .zip(&rows.pick)
                .map(|((i, pick), label)| {
                    (MenuItem { icon: Some(skills::pick_icon(c, pick)), ..MenuItem::new(label) }, Target::SkillItem(i))
                })
                .unzip(),
        }
    }

    fn node_rect(&self, key: NodeKey) -> Option<Rect> {
        self.motions.node(key).map(|m| m.rect())
    }

    fn place(&self, kind: PopoverKind, count: usize) -> Option<Placement> {
        let h = Menu::height(if kind == PopoverKind::Model { count.min(8) } else { count });
        let c = self.composer;
        let shape = |rect: Rect| GooShape::RoundRect { rect, radius: metrics::NODE_RADIUS };
        Some(match kind {
            PopoverKind::Permission => below_chip(c, self.permission_chip?, h),
            PopoverKind::Model => Placement {
                anchor: Point::new(c.x + 110.0, c.y),
                side: Side::Up,
                panel: Rect::new(c.x, c.y - GAP - h, 360.0, h),
                source: shape(Rect::new(c.x, c.y, c.w, 148.0)),
            },
            PopoverKind::Skills => Placement {
                anchor: Point::new(c.x, c.y + 66.0),
                side: Side::Left,
                panel: Rect::new(c.x - GAP - 210.0, c.y + 66.0 - h * 0.5, 210.0, h),
                source: shape(Rect::new(c.x, c.y, 401.0, c.h)),
            },
            PopoverKind::Branch => {
                let node = self.node_rect(NodeKey::Repository)?;
                Placement {
                    anchor: Point::new(node.center().x, node.bottom()),
                    side: Side::Down,
                    panel: Rect::new(node.x, node.bottom() + GAP, node.w, h),
                    source: shape(node),
                }
            }
            PopoverKind::Repository | PopoverKind::AttachRepository => {
                from_ear(self.node_rect(NodeKey::Repository)?, h)
            }
            PopoverKind::AddSkill => from_ear(self.node_rect(NodeKey::Skills)?, h),
            PopoverKind::PluginAccounts => {
                let node = self.node_rect(NodeKey::Connections)?;
                let open = self.session.account_menu.as_ref()?;
                let index = self.session.plugin_rows().iter().position(|row| row.plugin == open)?;
                let y = super::nodes::connections::row_centre(node, index);
                Placement {
                    anchor: Point::new(node.x, y),
                    side: Side::Left,
                    panel: Rect::new(node.x - GAP - ACCOUNT_PANEL, y - h * 0.5, ACCOUNT_PANEL, h),
                    source: shape(node),
                }
            }
            PopoverKind::PullRequests => {
                let node = self.node_rect(NodeKey::Repository)?;
                let y = super::nodes::repository::pull_requests_row_centre(node);
                Placement {
                    anchor: Point::new(node.x, y),
                    side: Side::Left,
                    panel: Rect::new(node.x - GAP - REPOSITORY_PANEL, y - h * 0.5, REPOSITORY_PANEL, h),
                    source: shape(node),
                }
            }
            PopoverKind::Logins | PopoverKind::SaveLogin | PopoverKind::Passkeys => {
                under_key(self.node_rect(NodeKey::Browser)?, h)
            }
        })
    }
}

fn under_key(card: Rect, h: f32) -> Placement {
    let key = BrowserCard::button(card, BrowserPart::Logins);
    Placement {
        anchor: Point::new(key.center().x, key.bottom()),
        side: Side::Down,
        panel: Rect::new(key.right() - LOGIN_PANEL, key.bottom() + GAP, LOGIN_PANEL, h),
        source: GooShape::RoundRect { rect: key, radius: metrics::CHIP_RADIUS },
    }
}

fn passkey_rows(session: &Session) -> Vec<String> {
    let Some(card) = session.browser() else { return Vec::new() };
    if card.passkey.as_ref().is_some_and(|p| p.ask.registration.is_some()) {
        let strings = session.strings();
        return session.vault_accounts().iter().map(|a| strings.t(Text::SavePasskeyTo(a))).collect();
    }
    card.passkeys().iter().map(passkey_label).collect()
}

fn passkey_label(passkey: &superfik_core::passkey::StoredPasskey) -> String {
    match &passkey.user_name {
        Some(user) => format!("{} · {user}", passkey.item),
        None => passkey.item.clone(),
    }
}

fn offer_label(offer: &crate::state::SaveOffer) -> String {
    let site = superfik_browser::address::site(&offer.url);
    if offer.username.is_empty() { site.to_owned() } else { format!("{} · {site}", offer.username) }
}

fn login_label(candidate: &superfik_core::autofill::Candidate) -> String {
    match &candidate.username {
        Some(username) => format!("{} · {username}", candidate.item),
        None => candidate.item.clone(),
    }
}

fn from_ear(node: Rect, h: f32) -> Placement {
    let ear = NodeFrame::ear_rect(node, EarSide::Left);
    let y = ear.y + ear.h * 0.5;
    let stem_x = ear.x - EAR_CLEARANCE;
    Placement {
        anchor: Point::new(stem_x, y),
        side: Side::Left,
        panel: Rect::new(stem_x - GAP - REPOSITORY_PANEL, y - h * 0.5, REPOSITORY_PANEL, h),
        source: GooShape::RoundRect { rect: ear, radius: NodeFrame::EAR_RADIUS },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_permission_menu_hangs_from_the_chip_wherever_it_is() {
        let composer = Rect::new(100.0, 200.0, 440.0, 190.0);
        for chip_x in [146.0, 300.0] {
            let chip = Rect::new(chip_x, 349.0, 120.0, 32.0);
            let place = below_chip(composer, chip, 150.0);
            assert_eq!(place.anchor.x, chip.center().x, "stem under the chip");
            assert_eq!(place.panel.x, chip.x, "menu starts at the chip");
            assert!(place.panel.y > composer.bottom(), "menu below the composer");
        }
    }
}
