use crate::api::features::schedules::{SchedulesReply, SchedulesRequest};
use agent_engine::tools::builtin::schedules;

use crate::features::context::Context;

pub fn handle(context: &Context<'_>, request: SchedulesRequest) -> Result<SchedulesReply, String> {
    let client = context.client;
    let mut due = Vec::new();
    match request {
        SchedulesRequest::List => {}
        SchedulesRequest::TakeDue => due = schedules::take_due(client, schedules::now()),
        SchedulesRequest::Delete { id } => {
            schedules::remove(client, &id);
        }
        SchedulesRequest::Create { prompt, when } => {
            let prompt = Some(prompt.trim()).filter(|prompt| !prompt.is_empty());
            schedules::add(client, prompt.ok_or("a scheduled prompt needs its text")?, when, schedules::now())?;
        }
    }
    Ok(SchedulesReply { due, scheduled: schedules::list(client) })
}

pub fn forget(client: u64) {
    schedules::forget_client(client);
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::schedule::When;

    fn ask(client: u64, request: SchedulesRequest) -> Result<SchedulesReply, String> {
        crate::features::context::testing::local(client, |context| handle(context, request))
    }

    #[test]
    fn a_client_takes_its_due_prompts_once_and_deletes_the_others() {
        let (client, other) = (77_001, 77_002);
        let now = schedules::now();
        schedules::add(client, "check the build", When::At(now - 1), now).expect("due");
        let later = schedules::add(client, "tomorrow", When::At(now + 86_400), now).expect("later");
        let listed = ask(client, SchedulesRequest::List).expect("listed");
        assert_eq!((listed.due.len(), listed.scheduled.len()), (0, 2), "listing takes nothing");
        let found = ask(client, SchedulesRequest::TakeDue).expect("found");
        assert_eq!(found.due.iter().map(|due| due.prompt.as_str()).collect::<Vec<_>>(), ["check the build"]);
        assert_eq!(found.scheduled, std::slice::from_ref(&later));
        assert!(ask(client, SchedulesRequest::TakeDue).expect("again").due.is_empty());
        assert!(ask(other, SchedulesRequest::List).expect("other").scheduled.is_empty());
        let unknown = ask(client, SchedulesRequest::Delete { id: "cron_none".into() }).expect("listed");
        assert_eq!(unknown.scheduled.len(), 1, "deleting what is not there only lists");
        let deleted = ask(client, SchedulesRequest::Delete { id: later.id }).expect("deleted");
        assert!(deleted.scheduled.is_empty() && deleted.due.is_empty());
        forget(client);
    }

    #[test]
    fn a_client_schedules_a_prompt_of_its_own() {
        let client = 77_003;
        let at = schedules::now() + 600;
        let create = |prompt: &str| ask(client, SchedulesRequest::Create { prompt: prompt.into(), when: When::At(at) });
        let kept = create("check the deploy").expect("made").scheduled;
        assert_eq!(kept.len(), 1);
        assert_eq!((kept[0].prompt.as_str(), &kept[0].when), ("check the deploy", &When::At(at)));
        assert!(create("  ").is_err(), "a prompt is needed");
        forget(client);
    }
}
