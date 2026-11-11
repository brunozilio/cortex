use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, clip, field, required};
use crate::model::ToolSpec;

const OUTPUT: usize = 4_000;

fn source(cell: &Value) -> String {
    match &cell["source"] {
        Value::Array(lines) => lines.iter().filter_map(Value::as_str).collect(),
        Value::String(text) => text.clone(),
        _ => String::new(),
    }
}

pub fn render(text: &str) -> Result<String, String> {
    let notebook: Value = serde_json::from_str(text).map_err(|e| format!("not a notebook: {e}"))?;
    let cells = notebook["cells"].as_array().ok_or("not a notebook: no cells")?;
    let mut out = String::new();
    for (at, cell) in cells.iter().enumerate() {
        let kind = cell["cell_type"].as_str().unwrap_or("code");
        let id = cell["id"].as_str().map(|id| format!(" id={id}")).unwrap_or_default();
        out.push_str(&format!("<cell {at}{id} {kind}>\n{}\n", source(cell).trim_end()));
        let mut printed = String::new();
        for output in cell["outputs"].as_array().into_iter().flatten() {
            let text = match &output["text"] {
                Value::Null => source(&json!({"source": output["data"]["text/plain"].clone()})),
                text => source(&json!({ "source": text.clone() })),
            };
            printed.push_str(&text);
            if output["data"]["image/png"].is_string() {
                printed.push_str("[an image]\n");
            }
        }
        if !printed.trim().is_empty() {
            out.push_str(&format!("<output>\n{}\n</output>\n", clip(printed.trim_end(), OUTPUT)));
        }
        out.push_str("</cell>\n");
    }
    Ok(out)
}

#[derive(Debug)]
pub struct NotebookEdit;

impl Tool for NotebookEdit {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "NotebookEdit".into(),
            description: "Change one cell of a Jupyter notebook (.ipynb): `replace` its source (the default), \
                          `insert` a new cell after it (or first, without `cell_number`), or `delete` it. Cells \
                          are numbered from 0 as Read shows them. Read the notebook first."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "notebook_path": { "type": "string" },
                    "cell_number": { "type": "integer", "description": "The cell, from 0." },
                    "new_source": { "type": "string" },
                    "cell_type": { "enum": ["code", "markdown"] },
                    "edit_mode": { "enum": ["replace", "insert", "delete"] }
                },
                "required": ["notebook_path"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Edits
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "notebook_path").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let path = match required(input, "notebook_path") {
            Ok(path) => workspace.resolve(path),
            Err(error) => return error,
        };
        if !workspace.has_read(&path) {
            return Outcome::error("Read the notebook before editing it.");
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => return Outcome::error(format!("Could not read {}: {error}", path.display())),
        };
        let edited = match edit(&text, input) {
            Ok(edited) => edited,
            Err(error) => return Outcome::error(error),
        };
        workspace.before_write(&path);
        match std::fs::write(&path, edited) {
            Ok(()) => Outcome::ok(format!("Updated {}.", path.display())),
            Err(error) => Outcome::error(format!("Could not write {}: {error}", path.display())),
        }
    }
}

fn edit(text: &str, input: &Value) -> Result<String, String> {
    let mut notebook: Value = serde_json::from_str(text).map_err(|e| format!("not a notebook: {e}"))?;
    let cells = notebook["cells"].as_array_mut().ok_or("not a notebook: no cells")?;
    let mode = field(input, "edit_mode").unwrap_or("replace");
    let number = input.get("cell_number").and_then(Value::as_u64).map(|n| n as usize);
    let new_source = field(input, "new_source").unwrap_or_default();
    let lines: Vec<Value> = new_source.split_inclusive('\n').map(|line| json!(line)).collect();
    match mode {
        "insert" => {
            let kind = field(input, "cell_type").unwrap_or("code");
            let mut cell = json!({ "cell_type": kind, "metadata": {}, "source": lines });
            if kind == "code" {
                cell["outputs"] = json!([]);
                cell["execution_count"] = Value::Null;
            }
            let at = number.map_or(0, |n| n + 1).min(cells.len());
            cells.insert(at, cell);
        }
        "delete" => {
            let at = number.ok_or("delete needs cell_number")?;
            if at >= cells.len() {
                return Err(format!("the notebook has {} cells", cells.len()));
            }
            cells.remove(at);
        }
        "replace" => {
            let at = number.ok_or("replace needs cell_number")?;
            let cell = cells.get_mut(at).ok_or(format!("the notebook has no cell {at}"))?;
            cell["source"] = json!(lines);
            if let Some(kind) = field(input, "cell_type") {
                cell["cell_type"] = json!(kind);
            }
            if cell["cell_type"] == "code" {
                cell["outputs"] = json!([]);
                cell["execution_count"] = Value::Null;
            }
        }
        other => return Err(format!("unknown edit_mode {other}")),
    }
    serde_json::to_string_pretty(&notebook).map(|text| text + "\n").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTEBOOK: &str = r##"{"cells":[
        {"cell_type":"markdown","id":"a","metadata":{},"source":["# Title\n"]},
        {"cell_type":"code","id":"b","metadata":{},"execution_count":1,"source":["print(1)\n","x = 2"],
         "outputs":[{"output_type":"stream","name":"stdout","text":["1\n"]}]}
    ],"metadata":{},"nbformat":4,"nbformat_minor":5}"##;

    #[test]
    fn cells_read_with_their_outputs() {
        assert_eq!(
            render(NOTEBOOK).expect("notebook"),
            "<cell 0 id=a markdown>\n# Title\n</cell>\n<cell 1 id=b code>\nprint(1)\nx = 2\n<output>\n1\n</output>\n</cell>\n"
        );
    }

    #[test]
    fn a_cell_is_replaced_inserted_or_deleted() {
        let replaced = edit(NOTEBOOK, &json!({"cell_number": 1, "new_source": "print(3)"})).expect("replace");
        assert!(render(&replaced).expect("read").contains("<cell 1 id=b code>\nprint(3)\n</cell>"), "outputs cleared");
        let inserted = edit(
            NOTEBOOK,
            &json!({"edit_mode": "insert", "cell_number": 0, "new_source": "y = 1", "cell_type": "code"}),
        )
        .expect("insert");
        assert!(render(&inserted).expect("read").contains("<cell 1 code>\ny = 1"));
        let deleted = edit(NOTEBOOK, &json!({"edit_mode": "delete", "cell_number": 0})).expect("delete");
        assert!(render(&deleted).expect("read").starts_with("<cell 0 id=b code>"));
        assert!(edit(NOTEBOOK, &json!({"edit_mode": "delete", "cell_number": 9})).is_err());
    }
}
