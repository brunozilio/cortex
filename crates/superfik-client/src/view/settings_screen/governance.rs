//! The Organization pane's governance groups, for owners and admins: the
//! policy (the loosest permission mode, the monthly budget, the models
//! allowed) and what each member spent this month. Pure.

use superfik_core::{ModelOption, PermissionMode};
use superfik_i18n::{Strings, Text};

use super::rows::{Control, Group, Line};
use crate::state::{OrganizationState, PolicyChoice, Setting};

const BUDGETS: [u32; 4] = [50, 100, 250, 500];

fn line(title: String, detail: String, control: Control) -> Line {
    Line { browser: None, title, detail: Some(detail), control }
}

pub fn policy(organization: &OrganizationState, models: &[ModelOption], strings: &Strings) -> Group {
    let t = |text: Text<'_>| strings.t(text);
    let policy = &organization.policy;
    let modes = [
        (t(Text::PermissionPlan), Some(PermissionMode::Plan)),
        (t(Text::PermissionAsk), Some(PermissionMode::Ask)),
        (t(Text::PermissionEdits), Some(PermissionMode::AcceptEdits)),
        (t(Text::PolicyAnyMode), None),
    ];
    let selected = modes.iter().position(|(_, mode)| *mode == policy.max_mode).unwrap_or(3);
    let options = modes.into_iter().map(|(name, mode)| (name, Setting::Policy(PolicyChoice::Mode(mode)))).collect();
    let mut lines = vec![line(
        t(Text::PolicyMaxMode),
        t(Text::PolicyMaxModeCaption),
        Control::Choice { options, selected, enabled: true },
    )];
    let budgets: Vec<Option<u32>> = std::iter::once(None).chain(BUDGETS.map(Some)).collect();
    let current = policy.monthly_budget_usd.map(|usd| usd as u32);
    let selected = budgets.iter().position(|budget| *budget == current).unwrap_or(0);
    let options = budgets
        .into_iter()
        .map(|budget| {
            let name = budget.map_or_else(|| t(Text::PolicyUnlimited), |dollars| format!("${dollars}"));
            (name, Setting::Policy(PolicyChoice::Budget(budget)))
        })
        .collect();
    lines.push(line(
        t(Text::PolicyBudgetTitle),
        t(Text::PolicyBudgetCaption),
        Control::Choice { options, selected, enabled: true },
    ));
    for (index, model) in models.iter().enumerate() {
        let on = policy.allows_model(&model.id);
        let flip = Setting::Policy(PolicyChoice::Model(index, !on));
        lines.push(line(model.label.clone(), t(Text::PolicyModelCaption), Control::Switch { on, flip }));
    }
    Group { heading: Some(t(Text::OrganizationPolicy)), lines }
}

pub fn usage(organization: &OrganizationState, strings: &Strings) -> Option<Group> {
    let lines: Vec<Line> = organization
        .usage
        .iter()
        .map(|member| {
            let dollars = format!("${:.2}", member.cost_usd);
            let detail = strings.t(Text::UsageLine(&dollars, member.runs as usize));
            line(member.name.clone(), detail, Control::None)
        })
        .collect();
    (!lines.is_empty()).then(|| Group { heading: Some(strings.t(Text::UsageThisMonth)), lines })
}

#[cfg(test)]
mod tests {
    use superfik_core::EngineKind;
    use superfik_core::policy::Policy;
    use superfik_i18n::Language;
    use superfik_protocol::MemberUsage;

    use super::*;

    fn models() -> Vec<ModelOption> {
        vec![
            ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5"),
            ModelOption::new(EngineKind::Native, "claude-sonnet-5-5", "Sonnet 5.5"),
        ]
    }

    #[test]
    fn the_policy_shows_what_is_set_and_what_each_line_changes() {
        let organization = OrganizationState {
            policy: Policy {
                models: Some(vec!["claude-sonnet-5-5".into()]),
                max_mode: Some(PermissionMode::AcceptEdits),
                monthly_budget_usd: Some(100.0),
            },
            ..OrganizationState::default()
        };
        let group = policy(&organization, &models(), &Strings::new(Language::English));
        let Control::Choice { selected, .. } = &group.lines[0].control else { panic!("modes") };
        assert_eq!(*selected, 2, "accept edits");
        let Control::Choice { options, selected, .. } = &group.lines[1].control else { panic!("budgets") };
        assert_eq!((options[*selected].0.as_str(), options.len()), ("$100", 5));
        assert_eq!(
            group.lines[2].control,
            Control::Switch { on: false, flip: Setting::Policy(PolicyChoice::Model(0, true)) }
        );
        assert!(matches!(group.lines[3].control, Control::Switch { on: true, .. }));
    }

    #[test]
    fn usage_lists_each_member_in_dollars_and_runs() {
        let ana = MemberUsage {
            user: superfik_core::UserId::new(),
            name: "Ana".into(),
            month: "2026-10".into(),
            cost_usd: 12.5,
            runs: 3,
        };
        let organization = OrganizationState { usage: vec![ana], ..OrganizationState::default() };
        let group = usage(&organization, &Strings::new(Language::English)).expect("listed");
        assert_eq!(group.lines[0].detail.as_deref(), Some("$12.50 · 3 runs"));
        assert!(usage(&OrganizationState::default(), &Strings::new(Language::English)).is_none());
    }
}
