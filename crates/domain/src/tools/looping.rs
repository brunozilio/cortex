pub fn loop_prompt(text: &str) -> Option<String> {
    let text = text.trim();
    let (first, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let (cron, task) = match every(first) {
        Some(cron) => (Some(cron), rest.trim()),
        None => (None, text),
    };
    if task.is_empty() {
        return None;
    }
    Some(match cron {
        Some((cron, said)) => format!(
            "Do this task now. Then schedule it with CronCreate (cron \"{cron}\", recurring, with the task as its \
             prompt) so it runs again {said}, and say how to stop it (CronDelete). Task: {task}"
        ),
        None => format!(
            "Do this task now. When you finish, unless the task is complete for good, call ScheduleWakeup with \
             a delay that fits what you are waiting for (60 to 3600 seconds) and this whole message as its \
             prompt, so you come back to it. Task: {task}"
        ),
    })
}

fn every(interval: &str) -> Option<(String, String)> {
    let unit = interval.chars().last()?;
    let n: u32 = interval[..interval.len() - unit.len_utf8()].parse().ok().filter(|n| *n > 0)?;
    let (cron, said) = match unit {
        's' => {
            let minutes = n.div_ceil(60).clamp(1, 59);
            (format!("*/{minutes} * * * *"), format!("every {minutes} minute(s)"))
        }
        'm' if n < 60 => (format!("*/{n} * * * *"), format!("every {n} minute(s)")),
        'm' => return every(&format!("{}h", n / 60)),
        'h' if n < 24 => (format!("0 */{n} * * *"), format!("every {n} hour(s)")),
        'h' => return every(&format!("{}d", n / 24)),
        'd' if n <= 31 => (format!("0 0 */{n} * *"), format!("every {n} day(s)")),
        _ => return None,
    };
    Some((cron, said))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_interval_becomes_a_cron_schedule() {
        let prompt = loop_prompt("5m check the deploy").expect("prompt");
        assert!(prompt.contains("cron \"*/5 * * * *\"") && prompt.ends_with("Task: check the deploy"), "{prompt}");
        assert!(loop_prompt("2h tidy").expect("prompt").contains("\"0 */2 * * *\""));
        assert!(loop_prompt("90s x").expect("prompt").contains("\"*/2 * * * *\""), "seconds round up to minutes");
        assert!(loop_prompt("120m x").expect("prompt").contains("\"0 */2 * * *\""));
    }

    #[test]
    fn without_an_interval_the_agent_paces_itself() {
        let prompt = loop_prompt("babysit the PR").expect("prompt");
        assert!(prompt.contains("ScheduleWakeup") && prompt.ends_with("Task: babysit the PR"), "{prompt}");
        assert_eq!(loop_prompt("5m"), None, "an interval and no task");
        assert_eq!(loop_prompt("  "), None);
    }
}
