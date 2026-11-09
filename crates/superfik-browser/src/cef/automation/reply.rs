//! Advances a native command only when its matching reply arrives.
use super::*;
impl Automation {
    pub fn reply(&mut self, id: u64, result: Result<Value, String>) {
        if self.expected != id {
            return;
        }
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.finish(Err(format!(
                    "Native browser operation failed: {error}. Take a new snapshot if the page changed."
                )));
                return;
            }
        };
        let state = std::mem::replace(&mut self.state, State::Idle);
        match state {
            State::Setup => self.send("Page.getFrameTree", json!({}), "", State::Frames),
            State::Frames => {
                self.frames = crate::devtools::frames(&result).into();
                for (frame, (_, url)) in &self.sessions {
                    if !self.frames.iter().any(|(id, _)| id == frame) {
                        self.frames.push_back((frame.clone(), url.clone()));
                    }
                }
                self.next_frame();
            }
            State::Ax => {
                self.refs.append(&result, &self.session, &self.frame, &mut self.lines);
                self.next_frame();
            }
            State::Scroll(node) => self.send(
                "DOM.resolveNode",
                json!({"backendNodeId":node.backend}),
                &node.session.clone(),
                State::Resolve(node),
            ),
            State::Resolve(node) => {
                let object = result["object"]["objectId"].as_str().unwrap_or("").to_owned();
                if object.is_empty() {
                    self.finish(Err("Stale ref; take a new snapshot.".into()));
                    return;
                }
                self.send("Runtime.callFunctionOn", json!({"objectId":object,"functionDeclaration":actions::CHECK,"arguments":[{"value":matches!(self.operation,Operation::Type(..))}],"returnByValue":true,"userGesture":true}), &node.session.clone(), State::Check(node, object));
            }
            State::Check(node, object) => {
                if let Err(error) = crate::devtools::value(&result) {
                    self.finish(Err(error));
                    return;
                }
                self.next += 1;
                let mut release = json!({"id":self.next,"method":"Runtime.releaseObject","params":{"objectId":object}});
                if !node.session.is_empty() {
                    release["sessionId"] = node.session.clone().into();
                }
                self.outbox.push(release.to_string());
                if let Operation::Upload(_, paths) = &self.operation {
                    if result["result"]["value"]["file"] == true {
                        self.send(
                            "DOM.setFileInputFiles",
                            json!({"backendNodeId":node.backend,"files":paths}),
                            &node.session,
                            State::Upload,
                        );
                    } else {
                        self.chooser = Some(node.clone());
                        self.send(
                            "Page.setInterceptFileChooserDialog",
                            json!({"enabled":true}),
                            &node.session.clone(),
                            State::Intercept(node),
                        );
                    }
                } else if let Operation::Type(_, text, submit) = &self.operation {
                    self.send(
                        "Input.insertText",
                        json!({"text":text}),
                        &node.session.clone(),
                        State::Insert(node, *submit),
                    );
                } else {
                    self.send(
                        "DOM.getContentQuads",
                        json!({"backendNodeId":node.backend}),
                        &node.session.clone(),
                        State::Quads(node),
                    );
                }
            }
            State::Quads(node) => {
                let quad = result["quads"].get(0).and_then(Value::as_array);
                let Some(quad) = quad.filter(|q| q.len() == 8) else {
                    self.finish(Err("Element has no visible bounds.".into()));
                    return;
                };
                let (x, y) = (
                    quad.iter().step_by(2).filter_map(Value::as_f64).sum::<f64>() / 4.,
                    quad.iter().skip(1).step_by(2).filter_map(Value::as_f64).sum::<f64>() / 4.,
                );
                self.mouse(x, y, &node.session);
            }
            State::Intercept(node) => self.send(
                "DOM.getContentQuads",
                json!({"backendNodeId":node.backend}),
                &node.session.clone(),
                State::Quads(node),
            ),
            State::Release if self.chooser.is_some() => {
                self.state = State::Chooser;
                self.expected = 0;
            }
            State::Insert(node, true) => {
                self.send(
                    "Input.dispatchKeyEvent",
                    json!({"type":"keyDown","key":"Enter","code":"Enter","windowsVirtualKeyCode":13,"text":"\r"}),
                    &node.session,
                    State::Release,
                );
                self.next += 1;
                let mut up = json!({"id":self.next,"method":"Input.dispatchKeyEvent","params":{"type":"keyUp","key":"Enter","code":"Enter","windowsVirtualKeyCode":13}});
                if !node.session.is_empty() {
                    up["sessionId"] = node.session.into();
                }
                self.outbox.push(up.to_string());
            }
            State::Insert(_, false) | State::Release | State::Upload | State::Dialog => {
                self.finish(Ok(BrowserAnswer::text("Native action completed.")))
            }
            State::Evaluate => self.finish(crate::devtools::value(&result).map(BrowserAnswer::text)),
            State::Idle | State::Chooser => {}
        }
    }
}
