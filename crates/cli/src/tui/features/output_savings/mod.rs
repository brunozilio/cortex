use domain::features::OUTPUT_SAVINGS;
use domain::output::report;

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["gain"],
            about: "How much shorter command output reached the model this session (experimental)",
            run: gain,
        }],
        ..Feature::default()
    }
}

fn gain(app: &mut App, _argument: Option<String>) {
    if !super::on(app, OUTPUT_SAVINGS) {
        return app.transcript.notice(turned_off());
    }
    app.spawn(|client| match client.output_savings() {
        Ok(savings) => Msg::Notice(report(&savings)),
        Err(error) => Msg::Error(error.to_string()),
    });
}

fn turned_off() -> String {
    format!("/gain is off. Turn it on with /features {OUTPUT_SAVINGS} on.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_says_how_to_turn_it_back_on() {
        assert_eq!(turned_off(), "/gain is off. Turn it on with /features experimental-output-savings on.");
        assert_eq!(feature().commands[0].names, ["gain"]);
    }
}
