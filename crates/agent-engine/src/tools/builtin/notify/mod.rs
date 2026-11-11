mod push_notification;

use crate::tools::Tool;

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(push_notification::PushNotification)]
}
