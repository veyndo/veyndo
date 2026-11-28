//! Agent surfaces: the operations graph and planner check over MCP, the evaluation runner wrapper, MCP client configuration and a stdio bridge.
use crate::Run;
use crate::failure::{Exit, Failure, Outcome};
use crate::output::{self, column};
use crate::profile::Kind;
use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use veyndo_sdk::RequestOptions;
use veyndo_sdk::tools::{
    MerchantOperationsCheckInput, MerchantOperationsCheckInputStepsItem,
    MerchantOperationsListInput,
};

/// `veyndo ops …`
#[derive(Subcommand, Debug)]
pub enum OpsCommand {
    /// List the operations your credential may call, with cost, risk and approval
    List {
        /// Only these operations (repeatable)
        #[arg(long = "name", value_name = "OPERATION")]
        names: Vec<String>,
        /// Include operations the credential may not call now
        #[arg(long)]
        all: bool,
    },
    /// Check a planned sequence of operation calls without running anything
    #[command(after_help = "\
plan.json is either {\"steps\":[{\"name\":\"merchant.products\",\"arguments\":{\"limit\":5}}, …]}
or the bare steps array. Each step gets a verdict: allowed, approval (a person must approve it),
blocked or unknown, with stable reason codes. Exit code 10 when the plan is not executable.")]
    Check {
        /// Plan file (`-` for stdin)
        plan: PathBuf,
    },
}

/// `veyndo agent …`
#[derive(Subcommand, Debug)]
pub enum AgentCommand {
    /// Run the agent evaluation benchmark against a local twin shop (repository checkout)
    #[command(after_help = "\
Runs scripts/agent_evaluation.py from a Veyndo repository checkout: it starts its own server,
copies the seeded shop into private sandboxes per scenario and grades the runs. Arguments after
`--` are passed through, for example `veyndo agent eval -- --only pricing-raise-lamp` or
`veyndo agent eval -- --check`. Needs python3, a built server and the test PostgreSQL container
(see docs/agent-evaluation.md). It never talks to your hosted shop.")]
    Eval {
        /// Repository checkout (default: VEYNDO_REPO, else the nearest parent with the runner)
        #[arg(long, env = "VEYNDO_REPO")]
        repo: Option<PathBuf>,
        /// Arguments for scripts/agent_evaluation.py
        #[arg(last = true)]
        args: Vec<String>,
    },
}

/// `veyndo mcp`
#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Mcp {
    #[command(subcommand)]
    command: Option<McpCommand>,
    /// Print the configuration for one client only
    #[arg(long, value_enum)]
    client: Option<McpClient>,
}

#[derive(Subcommand, Debug)]
enum McpCommand {
    /// Bridge MCP JSON-RPC between stdin/stdout and the server, with the stored credential
    Serve,
}

/// MCP clients with a configuration snippet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum McpClient {
    /// Claude Code (`claude mcp add`)
    ClaudeCode,
    /// Claude Desktop (`claude_desktop_config.json`, stdio through `veyndo mcp serve`)
    ClaudeDesktop,
    /// Cursor (`.cursor/mcp.json`)
    Cursor,
}

/// `veyndo ops …`
pub async fn ops(run: &Run, command: OpsCommand) -> Outcome {
    let client = run.context.client()?;
    match command {
        OpsCommand::List { names, all } => {
            let input = MerchantOperationsListInput {
                include_unavailable: all.then_some(true),
                names: (!names.is_empty()).then_some(names),
            };
            let listed = client.mcp().call(&input).await?;
            let rows = listed["operations"].as_array().cloned().unwrap_or_default();
            output::rows(
                run.format,
                &listed,
                &rows,
                &[
                    column("OPERATION", &["/name"]),
                    column("WRITES", &["/facts/writes"]),
                    column("RISK", &["/facts/risk"]),
                    column("COST", &["/cost"]),
                    column("APPROVAL", &["/requiresApproval"]),
                    column("ADMITTED", &["/admitted"]),
                ],
            );
            Ok(())
        }
        OpsCommand::Check { plan } => {
            let steps = read_plan(&plan)?;
            let checked = client
                .mcp()
                .call(&MerchantOperationsCheckInput { steps })
                .await?;
            report_check(run, &checked)
        }
    }
}

fn read_plan(path: &Path) -> Outcome<Vec<MerchantOperationsCheckInputStepsItem>> {
    let text = if path == Path::new("-") {
        std::io::read_to_string(std::io::stdin())?
    } else {
        std::fs::read_to_string(path)
            .map_err(|e| Failure::invalid(format!("Cannot read {}: {e}", path.display())))?
    };
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| Failure::invalid(format!("The plan is not valid JSON: {e}")))?;
    let steps = match value {
        Value::Array(_) => value,
        Value::Object(ref object) if object.contains_key("steps") => value["steps"].clone(),
        _ => {
            return Err(Failure::invalid(
                "The plan must be {\"steps\":[…]} or an array of steps",
            ));
        }
    };
    let steps: Vec<MerchantOperationsCheckInputStepsItem> =
        serde_json::from_value(steps).map_err(|e| {
            Failure::invalid(format!(
                "Every step needs a \"name\" and optional \"arguments\": {e}"
            ))
        })?;
    if steps.is_empty() {
        return Err(Failure::invalid("The plan has no steps"));
    }
    Ok(steps)
}

fn report_check(run: &Run, checked: &Value) -> Outcome {
    let rows = checked["steps"].as_array().cloned().unwrap_or_default();
    output::rows(
        run.format,
        checked,
        &rows,
        &[
            column("#", &["/index"]),
            column("OPERATION", &["/name"]),
            column("VERDICT", &["/verdict"]),
            column("REASONS", &["/reasons"]),
            column("UNMET", &["/unmetRequires"]),
            column("ARGUMENTS", &["/argumentError"]),
        ],
    );
    let executable = checked["executable"] == json!(true);
    if !run.json() {
        let approval = if checked["approvalRequired"] == json!(true) {
            "; some steps need a person's approval"
        } else {
            ""
        };
        println!(
            "{}{approval}",
            if executable {
                "Plan is executable"
            } else {
                "Plan is not executable"
            }
        );
    }
    if executable {
        Ok(())
    } else {
        Err(Failure::new(
            Exit::Findings,
            "The plan has blocked or unknown steps",
        ))
    }
}

/// The repository root: explicit, or the nearest parent of the working directory with the runner.
fn repository(explicit: Option<PathBuf>) -> Outcome<PathBuf> {
    const RUNNER: &str = "scripts/agent_evaluation.py";
    if let Some(repo) = explicit {
        return if repo.join(RUNNER).is_file() {
            Ok(repo)
        } else {
            Err(Failure::usage(format!(
                "{} has no {RUNNER}",
                repo.display()
            )))
        };
    }
    let start = std::env::current_dir()?;
    start
        .ancestors()
        .find(|dir| dir.join(RUNNER).is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            Failure::usage("Run inside a Veyndo repository checkout or pass --repo / VEYNDO_REPO")
        })
}

/// `veyndo agent …`
pub fn agent(_: &Run, command: AgentCommand) -> Outcome {
    let AgentCommand::Eval { repo, args } = command;
    let repo = repository(repo)?;
    let status = std::process::Command::new("python3")
        .arg("scripts/agent_evaluation.py")
        .args(&args)
        .current_dir(&repo)
        .status()
        .map_err(|e| Failure::new(Exit::Unavailable, format!("Cannot start python3: {e}")))?;
    match status.code() {
        Some(0) => Ok(()),
        Some(code) => Err(Failure::new(
            Exit::Findings,
            format!("The evaluation runner exited with {code}"),
        )),
        None => Err(Failure::new(
            Exit::Failure,
            "The evaluation runner was interrupted",
        )),
    }
}

fn snippets(run: &Run, endpoint: &str, shop: &str) -> Vec<(McpClient, String)> {
    let program = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| "veyndo".into());
    let headers = json!({"Authorization": "Bearer ${VEYNDO_API_KEY}", "x-tenant": shop});
    let desktop = json!({"mcpServers": {"veyndo": {"command": program, "args": ["mcp", "serve", "--shop", shop, "--server", run.context.server]}}});
    let cursor = json!({"mcpServers": {"veyndo": {"url": endpoint, "headers": headers}}});
    vec![
        (
            McpClient::ClaudeCode,
            format!(
                "claude mcp add --transport http veyndo {endpoint} --header \"Authorization: Bearer $VEYNDO_API_KEY\" --header \"x-tenant: {shop}\""
            ),
        ),
        (
            McpClient::ClaudeDesktop,
            serde_json::to_string_pretty(&desktop).unwrap_or_default(),
        ),
        (
            McpClient::Cursor,
            serde_json::to_string_pretty(&cursor).unwrap_or_default(),
        ),
    ]
}

fn title(client: McpClient) -> &'static str {
    match client {
        McpClient::ClaudeCode => "Claude Code (HTTP, key from VEYNDO_API_KEY):",
        McpClient::ClaudeDesktop => {
            "Claude Desktop, claude_desktop_config.json (stdio bridge with the stored credential):"
        }
        McpClient::Cursor => {
            "Cursor, .cursor/mcp.json (key from the VEYNDO_API_KEY environment variable):"
        }
    }
}

/// `veyndo mcp`: configuration snippets never contain the secret, only a reference to it.
pub async fn mcp(run: &Run, mcp: Mcp) -> Outcome {
    if let Some(McpCommand::Serve) = mcp.command {
        return serve(run).await;
    }
    let endpoint = format!("{}/mcp", run.context.server);
    let shop = run.context.shop.clone().unwrap_or_else(|| "<shop>".into());
    let all = snippets(run, &endpoint, &shop);
    let chosen: Vec<_> = all
        .iter()
        .filter(|(c, _)| mcp.client.is_none_or(|wanted| wanted == *c))
        .collect();
    if run.json() {
        let configs: serde_json::Map<String, Value> = chosen
            .iter()
            .map(|(c, text)| {
                (
                    format!("{c:?}"),
                    serde_json::from_str(text).unwrap_or_else(|_| json!(text)),
                )
            })
            .collect();
        output::json(
            &json!({"endpoint": endpoint, "shop": shop, "transport": "streamable-http (JSON-RPC POST)", "configs": configs}),
        );
        return Ok(());
    }
    println!("MCP endpoint: {endpoint}  (shop {shop}, send it as x-tenant)\n");
    for (client, text) in chosen {
        println!("{}\n{text}\n", title(*client));
    }
    if run
        .context
        .credential
        .as_ref()
        .is_some_and(|c| c.kind == Kind::Session)
    {
        output::note(
            "A personal session expires after 12 hours; give MCP clients a scoped integration key.",
        );
    }
    Ok(())
}

/// Forwards newline-delimited JSON-RPC messages; notifications get no reply, as MCP's stdio transport expects.
async fn serve(run: &Run) -> Outcome {
    let client = run.context.client()?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            writeln!(
                stdout,
                "{}",
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})
            )?;
            stdout.flush()?;
            continue;
        };
        let id = message.get("id").cloned();
        let reply = match client
            .protocols()
            .post_mcp(RequestOptions::json(message))
            .await
        {
            Ok(reply) => reply,
            Err(error) => {
                let failure = Failure::from(error);
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":failure.message}})
            }
        };
        if id.is_some() && !reply.is_null() {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_are_objects_with_steps_or_arrays() {
        let dir = std::env::temp_dir().join(format!("veyndo-cli-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let object = dir.join("object.json");
        std::fs::write(
            &object,
            r#"{"steps":[{"name":"merchant.products","arguments":{"limit":5}}]}"#,
        )
        .unwrap();
        assert_eq!(read_plan(&object).unwrap()[0].name, "merchant.products");
        let array = dir.join("array.json");
        std::fs::write(&array, r#"[{"name":"catalog.search"}]"#).unwrap();
        assert!(read_plan(&array).unwrap()[0].arguments.is_none());
        let empty = dir.join("empty.json");
        std::fs::write(&empty, r#"{"steps":[]}"#).unwrap();
        assert_eq!(read_plan(&empty).unwrap_err().exit, Exit::Invalid);
        let wrong = dir.join("wrong.json");
        std::fs::write(&wrong, r#"{"plan":1}"#).unwrap();
        assert_eq!(read_plan(&wrong).unwrap_err().exit, Exit::Invalid);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_blocked_plan_is_a_finding() {
        let run = Run {
            context: crate::profile::Context {
                server: "https://api.veyndo.ai".into(),
                shop: Some("s".into()),
                credential: None,
                from_environment: false,
            },
            format: output::Format::Json,
        };
        let blocked = json!({"steps":[{"index":0,"name":"x","verdict":"unknown","reasons":[]}],"executable":false});
        assert_eq!(
            report_check(&run, &blocked).unwrap_err().exit,
            Exit::Findings
        );
        assert!(report_check(&run, &json!({"steps":[],"executable":true})).is_ok());
    }

    #[test]
    fn mcp_snippets_reference_the_key_without_containing_it() {
        let run = Run {
            context: crate::profile::Context {
                server: "https://api.veyndo.ai".into(),
                shop: Some("s".into()),
                credential: Some(crate::profile::Credential {
                    kind: Kind::IntegrationKey,
                    secret: "very-secret-key".into(),
                    email: None,
                }),
                from_environment: false,
            },
            format: output::Format::Table,
        };
        for (_, text) in snippets(&run, "https://api.veyndo.ai/mcp", "s") {
            assert!(!text.contains("very-secret-key"));
        }
    }
}
