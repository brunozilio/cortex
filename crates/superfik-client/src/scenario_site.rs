//! The website's story, painted by the app itself: one everyday task (a
//! proposal for a client) from the request to the result. `site-<step>`
//! is English, `site-<step>-pt` Portuguese; the steps are `ask`, `agents`,
//! `approve`, `memory`, `review` and `result`.

use std::path::PathBuf;

use superfik_core::{
    Activity, AgentAsk, AgentStatus, AskId, AskKind, CustomAgent, EngineEvent, MemoryId, Repository, RunId, SubAgent,
    ToolCall, Turn, TurnResponse, UserMessage, WorkspaceSnapshot,
};
use superfik_protocol::ServerEvent;

use super::scenarios::SyntheticImage;
use crate::state::{Action, ActiveRun, MemoryField, Session};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    En,
    Pt,
}

impl Lang {
    fn pick(self, en: &'static str, pt: &'static str) -> &'static str {
        if self == Lang::En { en } else { pt }
    }
}

pub fn by_name(name: &str) -> Option<(Session, Vec<SyntheticImage>)> {
    let step = name.strip_prefix("site-")?;
    let (step, lang) = match step.strip_suffix("-pt") {
        Some(step) => (step, Lang::Pt),
        None => (step, Lang::En),
    };
    let s = match step {
        "ask" => ask(lang),
        "agents" => agents(lang),
        "approve" => approve(lang),
        "memory" => memory(lang),
        "review" => review(lang),
        "result" => result(lang),
        _ => return None,
    };
    Some((s, Vec::new()))
}

fn prompt(l: Lang) -> &'static str {
    l.pick("Prepare the Acme proposal and send it to Ana today.", "Prepare a proposta da Acme e envie para a Ana hoje.")
}

fn workspace(l: Lang) -> Session {
    let mut s = Session::new(Some(PathBuf::from("/work/clients")));
    s.snapshot = WorkspaceSnapshot {
        repository: Some(Repository {
            root: "/work/clients".into(),
            slug: "studio/clients".into(),
            head_short_sha: "4c2e9a1".into(),
            head_subject: l.pick("Q3 proposals", "Propostas do 3º tri").into(),
            branch: "main".into(),
            branches: vec!["main".into()],
            ..Repository::default()
        }),
    };
    s.branch = Some("main".into());
    s.available_agents =
        crew(l).into_iter().map(|name| CustomAgent { name: name.into(), description: String::new() }).collect();
    s
}

fn tools(l: Lang, count: usize) -> Vec<ToolCall> {
    let all = [
        ("WebSearch", l.pick("Acme growth plans 2026", "planos de crescimento da Acme 2026")),
        ("Read", l.pick("/work/clients/acme/meeting-notes.md", "/work/clients/acme/notas-da-reuniao.md")),
        ("Read", l.pick("/work/clients/pricing.xlsx", "/work/clients/precos.xlsx")),
        ("Agent", l.pick("Draft the proposal", "Escrever a proposta")),
        ("Write", l.pick("/work/clients/acme/proposal.pdf", "/work/clients/acme/proposta.pdf")),
        ("Gmail", l.pick("Send the proposal to Ana", "Enviar a proposta para a Ana")),
    ];
    all.iter().take(count).enumerate().map(|(i, (name, what))| ToolCall::new(i.to_string(), *name, *what)).collect()
}

fn ask(l: Lang) -> Session {
    let mut s = workspace(l);
    s.conversation_mut().composer.set(prompt(l));
    s
}

fn crew(l: Lang) -> [&'static str; 3] {
    [l.pick("researcher", "pesquisador"), l.pick("pricing", "financeiro"), l.pick("writer", "redator")]
}

fn team(s: &Session, l: Lang) -> Vec<SubAgent> {
    let spawn = |i, kind, task: &str| SubAgent::spawned(i, kind, task, "", &s.available_agents);
    let [research, pricing, writer] = crew(l);
    let mut agents = vec![
        spawn(0, research, l.pick("Learn what Acme needs", "Entender o que a Acme precisa")),
        spawn(1, pricing, l.pick("Price the plan", "Montar o preço")),
        spawn(2, writer, l.pick("Draft the proposal", "Escrever a proposta")),
    ];
    agents[0].status = AgentStatus::Running { progress: 0.8 };
    agents[0].summary = l.pick("Reading the meeting notes", "Lendo as notas da reunião").into();
    agents[1].status = AgentStatus::Running { progress: 0.5 };
    agents[1].summary = l.pick("Comparing last year's deals", "Comparando os negócios do ano").into();
    agents[2].status = AgentStatus::Running { progress: 0.2 };
    agents[2].summary = l.pick("Outlining the proposal", "Estruturando a proposta").into();
    agents
}

fn agents(l: Lang) -> Session {
    let mut s = workspace(l);
    let team = team(&s, l);
    let c = s.conversation_mut();
    c.title = Some(prompt(l).into());
    c.started_unix = Some(0);
    c.agents = team;
    let mut turn =
        Turn::new(UserMessage { text: prompt(l).into(), images: Vec::new(), branch_caption: "clients/main".into() });
    turn.response = TurnResponse {
        activity: Activity::Writing,
        text: l
            .pick(
                "On it. One agent reads what Acme asked for, one prices the plan and one drafts the proposal.",
                "Já comecei. Um agente lê o que a Acme pediu, outro monta o preço e outro escreve a proposta.",
            )
            .into(),
        tools: tools(l, 3),
        elapsed_ms: 0,
        failure: None,
        usage: Vec::new(),
        structured: None,
    };
    c.turns.push(turn);
    c.run = Some(ActiveRun { id: RunId::new(), started_at: -18.0 });
    s
}

fn approve(l: Lang) -> Session {
    let mut s = agents(l);
    let Some(run) = s.conversation().run.as_ref().map(|r| r.id) else { return s };
    let kind = AskKind::Permission {
        tool: "Gmail".into(),
        subject: l
            .pick(
                "Send “Proposal for Acme” to ana@acme.com with proposal.pdf attached",
                "Enviar “Proposta para a Acme” para ana@acme.com com proposta.pdf anexada",
            )
            .into(),
        rememberable: true,
    };
    let event = EngineEvent::Asked(AgentAsk { id: AskId("site".into()), kind });
    s.apply(Action::Server(ServerEvent::Run { run, event }), 0.0);
    s
}

fn memory(l: Lang) -> Session {
    let mut s = agents(l);
    s.memory.memories = super::scenario_site_memory::memories(l == Lang::En);
    s.memory.loaded = true;
    s.memory.open = true;
    s.memory.focus = Some(MemoryField::Search);
    s.memory.query.set("Acme");
    let scores = superfik_core::memory::activate("Acme", &s.memory.memories);
    s.memory.lit = s.memory.memories.iter().zip(scores).filter(|(_, a)| *a > 0.0).map(|(m, a)| (m.id, a)).collect();
    s.memory.learned = vec![MemoryId(0)];
    s.memory.learned_at = 0.0;
    s.memory.refresh();
    s
}

const PROPOSAL_EN: &str = "Here is the proposal for Acme.\n\n\
    | Item | Value |\n|---|---|\n| Onboarding for three teams | $12,000 |\n\
    | Yearly plan, billed quarterly | $32,000 |\n| Priority support | $4,000 |\n| **Total** | **$48,000** |\n\n\
    Billing is quarterly, as Acme prefers.";
const PROPOSAL_PT: &str = "Aqui está a proposta para a Acme.\n\n\
    | Item | Valor |\n|---|---|\n| Implantação para três times | R$ 12.000 |\n\
    | Plano anual, pago por trimestre | R$ 32.000 |\n| Suporte prioritário | R$ 4.000 |\n| **Total** | **R$ 48.000** |\n\n\
    O pagamento é trimestral, como a Acme prefere.";

fn finished(s: &mut Session, l: Lang, text: &str, steps: usize) {
    let c = s.conversation_mut();
    for (agent, seconds) in c.agents.iter_mut().zip([14, 9, 22]) {
        agent.set_status(AgentStatus::Done { seconds });
        agent.summary = String::new();
    }
    c.agents[0].summary = l.pick("Acme wants three teams", "A Acme quer três times").into();
    c.agents[1].summary = l.pick("$48,000 a year", "R$ 48.000 por ano").into();
    c.agents[2].summary = l.pick("Proposal ready", "Proposta pronta").into();
    c.run = None;
    c.finished_at = Some(0.0);
    if let Some(turn) = c.turns.last_mut() {
        turn.response.activity = Activity::Done;
        turn.response.text = text.into();
        turn.response.tools = tools(l, steps);
        turn.response.elapsed_ms = 74_000;
    }
}

fn review(l: Lang) -> Session {
    let mut s = agents(l);
    finished(&mut s, l, l.pick(PROPOSAL_EN, PROPOSAL_PT), 5);
    s
}

fn result(l: Lang) -> Session {
    let mut s = agents(l);
    let text = l.pick(
        "Sent to Ana at 16:42 with proposal.pdf.\n\n- **$48,000** a year, billed quarterly.\n- A follow-up is set for Thursday.",
        "Enviada para a Ana às 16:42 com a proposta.pdf.\n\n- **R$ 48.000** por ano, pagos por trimestre.\n- Follow-up marcado para quinta.",
    );
    finished(&mut s, l, text, 6);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_exists_in_both_languages() {
        for step in ["ask", "agents", "approve", "memory", "review", "result"] {
            assert!(by_name(&format!("site-{step}")).is_some(), "{step}");
            assert!(by_name(&format!("site-{step}-pt")).is_some(), "{step}-pt");
        }
        assert!(by_name("site-nope").is_none());
    }

    #[test]
    fn the_story_moves_from_request_to_result() {
        let (ask, _) = by_name("site-ask").expect("ask");
        assert_eq!(ask.conversation().composer.text(), prompt(Lang::En));
        let (approve, _) = by_name("site-approve-pt").expect("approve");
        assert!(approve.pending_ask().is_some());
        let (memory, _) = by_name("site-memory").expect("memory");
        assert!(memory.memory.open && memory.memory.lit.iter().any(|(id, _)| *id == MemoryId(0)));
        let (result, _) = by_name("site-result").expect("result");
        assert!(!result.is_running());
        assert!(result.conversation().agents.iter().all(|a| matches!(a.status, AgentStatus::Done { .. })));
    }
}
