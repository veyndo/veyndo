//! Shared wire description for native operations and installed app actions; hints never grant execution rights.
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
/// The wire description of one tool: its schemas and safety hints.
pub struct ToolContract<'a> {
    name: &'a str,
    description: &'a str,
    input_schema: &'a Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_schema: Option<&'a Value>,
    read_only: bool,
    annotations: Safety,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Safety {
    read_only_hint: bool,
    destructive_hint: bool,
    idempotent_hint: bool,
    open_world_hint: bool,
}
impl<'a> ToolContract<'a> {
    /// Describes a tool; a tool that is not read-only is announced as destructive.
    pub fn describe(
        name: &'a str,
        description: &'a str,
        input: &'a Value,
        output: Option<&'a Value>,
        read_only: bool,
    ) -> Value {
        json!(Self {
            name,
            description,
            input_schema: input,
            output_schema: output,
            read_only,
            annotations: Safety {
                read_only_hint: read_only,
                destructive_hint: !read_only,
                idempotent_hint: false,
                open_world_hint: true
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ToolContract;
    use serde_json::json;

    #[test]
    fn a_tool_announces_its_schemas_and_whether_it_only_reads() {
        let input = json!({"type":"object"});
        let output = json!({"type":"object","properties":{"ok":{"type":"boolean"}}});
        let read = ToolContract::describe("orders.list", "Lists orders", &input, None, true);
        assert_eq!(read["name"], "orders.list");
        assert_eq!(read["inputSchema"], input);
        assert!(read.get("outputSchema").is_none());
        assert_eq!(read["readOnly"], true);
        assert_eq!(read["annotations"]["readOnlyHint"], true);
        assert_eq!(read["annotations"]["destructiveHint"], false);
        let write =
            ToolContract::describe("orders.cancel", "Cancels", &input, Some(&output), false);
        assert_eq!(write["outputSchema"], output);
        assert_eq!(write["annotations"]["destructiveHint"], true);
        assert_eq!(write["annotations"]["openWorldHint"], true);
    }
}
