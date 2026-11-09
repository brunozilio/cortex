//! Starts one queued operation on the selected native page.
use super::*;
impl Page {
    pub(super) fn start(&mut self, op: BrowserOp, now: Instant) -> Result<Step, String> {
        let load = Step::Load {
            started: false,
            revision: self.load_revision,
            start_by: now + LOAD_START,
            deadline: now + LOAD_LIMIT,
        };
        Ok(match op {
            BrowserOp::Navigate { url } => {
                let url = address::resolve(&url).ok_or("Give an address.")?;
                self.may_go(&url)?;
                self.failure = None;
                self.producer.navigate_to_url(&url).map_err(|e| e.to_string())?;
                Step::Load {
                    started: false,
                    revision: self.load_revision,
                    start_by: now + LOAD_LIMIT,
                    deadline: now + LOAD_LIMIT,
                }
            }
            BrowserOp::Back if !self.history.0 => return Err("There is nothing to go back to.".into()),
            BrowserOp::Forward if !self.history.1 => return Err("There is nothing to go forward to.".into()),
            BrowserOp::Back => {
                self.failure = None;
                log(self.producer.go_back());
                load
            }
            BrowserOp::Forward => {
                self.failure = None;
                log(self.producer.go_forward());
                load
            }
            BrowserOp::Reload => {
                self.failure = None;
                log(self.producer.reload());
                load
            }
            BrowserOp::Snapshot => self.native(Operation::Snapshot(0), now)?,
            BrowserOp::Dialog { accept, prompt } => {
                if prompt.as_ref().is_some_and(|p| p.len() > 4096) || (prompt.is_some() && accept.is_none()) {
                    return Err("A prompt needs an explicit accept decision and at most 4096 bytes.".into());
                }
                match accept {
                    None => Step::Done(Ok(self.automation.dialog_summary())),
                    Some(accept) => self.native(Operation::Dialog(accept, prompt), now)?,
                }
            }
            BrowserOp::Files => Step::Done(self.files.summary()),
            BrowserOp::Upload { reference, paths } => {
                self.native(Operation::Upload(reference, self.files.uploads(&paths)?), now)?
            }
            BrowserOp::SnapshotPage { offset } => self.native(Operation::Snapshot(offset as usize), now)?,
            BrowserOp::Wait(query) => Step::Wait(crate::cef::waiting::Waiting::new(query, now)?),
            BrowserOp::Tabs | BrowserOp::SwitchTab { .. } | BrowserOp::CloseTab { .. } => {
                return Err("Tab operations are routed by the host.".into());
            }
            BrowserOp::Evaluate { script } => self.native(Operation::Evaluate(script), now)?,
            BrowserOp::FillLogin { .. } => return Err("Signing in goes through the vault.".into()),
            BrowserOp::Click { target: Locator::Ref(reference) } => self.native(Operation::Click(reference), now)?,
            BrowserOp::Click { target: Locator::Point { x, y } } => {
                self.click(x, y);
                settle(format!("Clicked at {x},{y}."), now)
            }
            BrowserOp::Type { target: Some(Locator::Ref(reference)), text, submit } => {
                self.native(Operation::Type(reference, text, submit), now)?
            }
            BrowserOp::Type { target, text, submit } => {
                if let Some((x, y)) = target.and_then(agent::point) {
                    self.click(x, y);
                }
                self.type_text(&text, submit);
                if submit { lead("Typed.".to_owned(), now) } else { settle("Typed.".to_owned(), now) }
            }
            BrowserOp::Key { key } => {
                let down = agent::named_key(&key).ok_or_else(|| format!("Unknown key `{key}`."))?;
                let enter = down.code == 0x0D;
                self.press(down);
                let text = format!("Pressed {key}.");
                if enter { lead(text, now) } else { settle(text, now) }
            }
            BrowserOp::Scroll { dx, dy } => {
                let scale = self.producer.scale_factor();
                let (w, h) = (self.size.0 as i32, self.size.1 as i32);
                let (dx, dy) = ((-dx as f32 * scale) as i32, (-dy as f32 * scale) as i32);
                let modifiers = Modifiers::default();
                self.input(&PageInput::Wheel { x: w / 2, y: h / 2, dx, dy, modifiers });
                settle("Scrolled.".to_owned(), now)
            }
            BrowserOp::Screenshot => match self.producer.request_snapshot_png() {
                Ok(id) => Step::Shot { id, deadline: now + SCRIPT_LIMIT },
                Err(error) => return Err(format!("Could not capture the page: {error}")),
            },
        })
    }

    fn native(&mut self, operation: Operation, now: Instant) -> Result<Step, String> {
        let action = matches!(operation, Operation::Click(_) | Operation::Type(..));
        self.automation.begin(operation)?;
        self.flush_automation();
        Ok(Step::Native { deadline: now + SCRIPT_LIMIT, action })
    }
}
