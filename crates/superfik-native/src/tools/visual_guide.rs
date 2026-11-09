//! `VisualGuide`: the detailed rules for one kind of visual (the SVG the
//! agent draws in its reply), loaded when it is about to draw one, so the
//! system prompt carries only the essentials (`prompt::VISUALS`).

use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, required};
use crate::model::ToolSpec;

pub const VISUAL_GUIDE: &str = "VisualGuide";

const MODULES: [(&str, &str); 3] = [
    ("diagram", include_str!("../prompt/visuals/diagram.md")),
    ("chart", include_str!("../prompt/visuals/chart.md")),
    ("art", include_str!("../prompt/visuals/art.md")),
];

#[derive(Debug)]
pub struct VisualGuide;

impl Tool for VisualGuide {
    fn spec(&self) -> ToolSpec {
        let names: Vec<&str> = MODULES.iter().map(|(name, _)| *name).collect();
        ToolSpec {
            name: VISUAL_GUIDE.into(),
            description: "Load the detailed rules and examples for drawing one kind of visual (a ```visual block \
                          in your reply). Call it before your first visual of that kind in the conversation."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "module": {
                        "type": "string",
                        "enum": names,
                        "description": "diagram: flowcharts, structural and illustrative diagrams; chart: data as SVG; art: illustration."
                    }
                },
                "required": ["module"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        super::field(input, "module").map_or_else(|| "Visual guide".into(), |m| format!("Visual guide: {m}"))
    }

    fn run(&self, input: &Value, _: &Workspace) -> Outcome {
        let module = match required(input, "module") {
            Ok(module) => module,
            Err(error) => return error,
        };
        match MODULES.iter().find(|(name, _)| *name == module) {
            Some((_, text)) => Outcome::ok(*text),
            None => {
                let names: Vec<&str> = MODULES.iter().map(|(name, _)| *name).collect();
                Outcome::error(format!("Unknown module `{module}`. Choose one of: {}.", names.join(", ")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_each_module_and_refuses_others() {
        let workspace = Workspace::new(".");
        let diagram = VisualGuide.run(&json!({"module": "diagram"}), &workspace);
        assert!(!diagram.is_error && diagram.content.starts_with("# Diagrams"));
        for (name, _) in MODULES {
            assert!(!VisualGuide.run(&json!({ "module": name }), &workspace).is_error, "{name}");
        }
        assert!(VisualGuide.run(&json!({"module": "html"}), &workspace).is_error);
        assert!(VisualGuide.run(&json!({}), &workspace).is_error);
    }

    #[test]
    fn modules_are_written_for_superfiks_width_and_name_no_other_product() {
        for (name, text) in MODULES {
            for other in ["Claude", "Anthropic", "claude.ai", "imagine_", "680"] {
                assert!(!text.contains(other), "{name} names {other}");
            }
        }
    }
}
