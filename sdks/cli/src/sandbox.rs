//! Private sandboxes over the existing staging API: create, list, review the diff and release selected changes with explicit approval.
use crate::Run;
use crate::failure::{Exit, Failure, Outcome};
use crate::output::{self, column};
use clap::Subcommand;
use serde_json::{Value, json};
use std::io::{BufRead, IsTerminal};
use veyndo_sdk::RequestOptions;

/// The server releases at most this many reviewed changes per approval.
const RELEASE_BATCH: usize = 50;

/// `veyndo sandbox …`
#[derive(Subcommand, Debug)]
pub enum SandboxCommand {
    /// List the private sandboxes of the selected shop and recent releases
    List,
    /// Clone the live shop into a new private sandbox
    Create {
        /// Sandbox name
        #[arg(long, default_value = "CLI sandbox")]
        name: String,
    },
    /// Show what a sandbox would change in the live shop
    Diff {
        /// Sandbox ID
        sandbox: String,
    },
    /// Publish reviewed sandbox changes to the live shop
    #[command(after_help = "\
Without --key every non-conflicting change is released. The changes are listed first and
released only after you confirm, or with --yes. Each release is bound to the digest of the
change you reviewed; a change edited in between is refused (exit 7).")]
    Release {
        /// Sandbox ID
        sandbox: String,
        /// Release only these change keys (repeatable; see `veyndo sandbox diff`)
        #[arg(long = "key", value_name = "KEY")]
        keys: Vec<String>,
        /// Approve without the interactive confirmation
        #[arg(long)]
        yes: bool,
    },
}

/// Creates a sandbox of the selected shop and returns its ID.
pub async fn create(run: &Run, name: &str) -> Outcome<String> {
    let created = run
        .context
        .client()?
        .studio()
        .post_environments(RequestOptions::json(json!({"name": name})))
        .await?;
    created["id"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| Failure::new(Exit::Failure, "The server returned no sandbox ID"))
}

/// Confirms that `sandbox` is a sandbox of the selected shop (the server checks ownership again on every call).
pub async fn known(run: &Run, sandbox: &str) -> Outcome {
    let listed = run
        .context
        .client()?
        .studio()
        .get_environments(RequestOptions::default())
        .await?;
    let found = listed["environments"]
        .as_array()
        .is_some_and(|all| all.iter().any(|e| e["id"] == json!(sandbox)));
    if found {
        Ok(())
    } else {
        Err(Failure::new(
            Exit::NotFound,
            format!(
                "No sandbox {sandbox:?} in shop {}; see `veyndo sandbox list`",
                run.context.require_shop()?
            ),
        ))
    }
}

/// The next steps after writing into a sandbox.
pub fn next_steps(sandbox: &str) {
    output::note(&format!(
        "Review with `veyndo sandbox diff {sandbox}`, then publish with `veyndo sandbox release {sandbox}`."
    ));
}

/// `veyndo sandbox …`
pub async fn run(run: &Run, command: SandboxCommand) -> Outcome {
    match command {
        SandboxCommand::List => {
            let listed = run
                .context
                .client()?
                .studio()
                .get_environments(RequestOptions::default())
                .await?;
            let rows = listed["environments"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            output::rows(
                run.format,
                &listed,
                &rows,
                &[
                    column("ID", &["/id"]),
                    column("NAME", &["/name"]),
                    column("CREATED", &["/createdAt"]),
                ],
            );
            Ok(())
        }
        SandboxCommand::Create { name } => {
            let id = create(run, &name).await?;
            if run.json() {
                output::json(&json!({"id": id, "name": name}));
            } else {
                println!("Created sandbox {id}");
            }
            Ok(())
        }
        SandboxCommand::Diff { sandbox } => {
            let diff = diff(run, &sandbox).await?;
            let rows = rows(&diff);
            output::rows(
                run.format,
                &diff,
                &rows,
                &[
                    column("KEY", &["/key"]),
                    column("CONFLICT", &["/conflict"]),
                    column("DIGEST", &["/digest"]),
                ],
            );
            Ok(())
        }
        SandboxCommand::Release { sandbox, keys, yes } => release(run, &sandbox, &keys, yes).await,
    }
}

async fn diff(run: &Run, sandbox: &str) -> Outcome<Value> {
    Ok(run
        .context
        .client()?
        .studio()
        .get_environments_by_id_diff(sandbox, RequestOptions::default())
        .await?)
}

fn rows(diff: &Value) -> Vec<Value> {
    diff["changes"].as_array().cloned().unwrap_or_default()
}

/// The changes to release: the named keys (each must exist and be free of conflicts) or every clean change.
fn selection(changes: &[Value], keys: &[String]) -> Outcome<Vec<Value>> {
    if keys.is_empty() {
        return Ok(changes
            .iter()
            .filter(|c| c["conflict"] != json!(true))
            .cloned()
            .collect());
    }
    keys.iter()
        .map(|key| {
            let change = changes
                .iter()
                .find(|c| c["key"] == json!(key))
                .ok_or_else(|| {
                    Failure::new(Exit::NotFound, format!("The sandbox has no change {key:?}"))
                })?;
            if change["conflict"] == json!(true) {
                return Err(Failure::new(
                    Exit::Conflict,
                    format!("{key} changed in the live shop too; resolve it in Studio"),
                ));
            }
            Ok(change.clone())
        })
        .collect()
}

fn confirm(question: &str) -> Outcome<bool> {
    if !std::io::stdin().is_terminal() {
        return Err(Failure::usage(
            "Confirmation needs a terminal; review the diff and pass --yes",
        ));
    }
    eprint!("{question} [y/N] ");
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}

async fn release(run: &Run, sandbox: &str, keys: &[String], yes: bool) -> Outcome {
    let diff = diff(run, sandbox).await?;
    let chosen = selection(&rows(&diff), keys)?;
    if chosen.is_empty() {
        return Err(Failure::new(
            Exit::Findings,
            "Nothing to release: the sandbox has no change free of conflicts",
        ));
    }
    let shop = run.context.require_shop()?;
    if !run.json() {
        for change in &chosen {
            println!("  {}", output::cell(change.get("key")));
        }
    }
    if !yes
        && !confirm(&format!(
            "Release {} change(s) from {sandbox} to the live shop {shop}?",
            chosen.len()
        ))?
    {
        return Err(Failure::new(
            Exit::Failure,
            "Release cancelled; nothing was published",
        ));
    }
    let client = run.context.client()?;
    let mut releases = Vec::new();
    for batch in chosen.chunks(RELEASE_BATCH) {
        let selections: Vec<Value> = batch
            .iter()
            .map(|c| json!({"key": c["key"], "digest": c["digest"]}))
            .collect();
        let released = client
            .studio()
            .post_environments_by_id_release(
                sandbox,
                RequestOptions::json(json!({"approve": true, "selections": selections})),
            )
            .await?;
        releases.push(released);
    }
    if run.json() {
        output::json(&json!({"sandbox": sandbox, "releases": releases}));
    } else {
        println!(
            "Released {} change(s) to {shop} in {} release(s)",
            chosen.len(),
            releases.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_skips_conflicts_and_checks_named_keys() {
        let changes = [
            json!({"key":"product:a","digest":"1","conflict":false}),
            json!({"key":"settings","digest":"2","conflict":true}),
        ];
        assert_eq!(selection(&changes, &[]).unwrap().len(), 1);
        assert_eq!(
            selection(&changes, &["product:a".into()]).unwrap()[0]["digest"],
            "1"
        );
        assert_eq!(
            selection(&changes, &["settings".into()]).unwrap_err().exit,
            Exit::Conflict
        );
        assert_eq!(
            selection(&changes, &["nope".into()]).unwrap_err().exit,
            Exit::NotFound
        );
    }
}
