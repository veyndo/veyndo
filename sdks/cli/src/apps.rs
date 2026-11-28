//! App developer loop over the native app API: scaffold from a pattern, review (`/api/apps/review`), install into a private sandbox and read the metadata-only activity log.
use crate::failure::{Exit, Failure, Outcome};
use crate::output::{self, column};
use crate::templates::{self, TEMPLATES};
use crate::{Run, here, sandbox};
use clap::Subcommand;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use veyndo_sdk::{Client, RequestOptions};

/// `veyndo app …`
#[derive(Subcommand, Debug)]
pub enum AppCommand {
    /// Create an app project from a template (without a template: list them)
    New {
        /// Template name (advisor, returns, erp, edi, furniture)
        template: Option<String>,
        /// Project directory (default: the app ID)
        directory: Option<PathBuf>,
        /// Manifest ID (default: derived from the directory name)
        #[arg(long)]
        id: Option<String>,
    },
    /// Check the package locally, then review it on the server without installing
    #[command(after_help = "\
The server review returns the canonical digest, the permissions the package asks for and the
permissions it adds to an installed version. Nothing is written. --offline only checks the
files. Exit code 10 means the package has findings.")]
    Validate {
        /// Project directory or manifest.json (default: the current directory)
        path: Option<PathBuf>,
        /// Skip the server review
        #[arg(long)]
        offline: bool,
        /// Review against this sandbox instead of the live shop
        #[arg(long)]
        sandbox: Option<String>,
    },
    /// Install the package into a private sandbox (never the live shop)
    #[command(after_help = "\
The package is reviewed and installed with the exact reviewed digest and permissions, the same
explicit approval Studio asks for. A signed package (Manifest `distribution` plus a signed
contract.json) is verified by the server against the operator's registered publisher keys; sign
it offline first. Publish the sandbox with `veyndo sandbox release` after testing it.")]
    Deploy {
        /// Project directory or manifest.json (default: the current directory)
        path: Option<PathBuf>,
        /// Install into this sandbox (default: create a new one)
        #[arg(long)]
        sandbox: Option<String>,
    },
    /// Show the app's recent calls and event deliveries (metadata only, no payloads)
    Logs {
        /// App ID
        app: String,
        /// Read a sandbox instead of the live shop
        #[arg(long)]
        sandbox: Option<String>,
        /// Keep polling for new entries every few seconds
        #[arg(long)]
        follow: bool,
    },
}

/// A Manifest and its optional package contract, read from a project.
struct Package {
    manifest: Value,
    contract: Option<Value>,
    root: PathBuf,
}

fn read_json(path: &Path) -> Outcome<Value> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Failure::invalid(format!("Cannot read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| Failure::invalid(format!("{} is not valid JSON: {e}", path.display())))
}

fn load(path: &Path) -> Outcome<Package> {
    let (root, manifest) = if path.is_dir() {
        (path.to_path_buf(), path.join("manifest.json"))
    } else {
        (
            path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            path.to_path_buf(),
        )
    };
    let contract_path = root.join("contract.json");
    Ok(Package {
        manifest: read_json(&manifest)?,
        contract: if contract_path.is_file() {
            Some(read_json(&contract_path)?)
        } else {
            None
        },
        root,
    })
}

fn semver(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// File-level findings; the server compiler is the authority for everything else.
fn findings(package: &Package) -> Vec<String> {
    let m = &package.manifest;
    let mut found = Vec::new();
    if !m.is_object() {
        return vec!["manifest.json must be a JSON object".into()];
    }
    let id = m["id"].as_str().unwrap_or_default();
    if !templates::valid_id(id) {
        found.push(format!("id {id:?} must be 2-64 lowercase letters, digits or underscores, starting with a letter"));
    }
    let version = m["version"].as_str().unwrap_or_default();
    if !semver(version) {
        found.push(format!(
            "version {version:?} must be a semantic version such as 1.0.0"
        ));
    }
    if !m["name"]["en"].is_string() {
        found.push("name needs at least an English (en) entry".into());
    }
    match m["permissions"].as_array() {
        Some(list) if list.iter().all(Value::is_string) => {
            let unique: HashSet<_> = list.iter().filter_map(Value::as_str).collect();
            if unique.len() != list.len() {
                found.push("permissions lists a permission twice".into());
            }
        }
        _ => found.push("permissions must be a list of strings".into()),
    }
    if let Some(contract) = &package.contract {
        if contract["app"].as_str() != Some(id) || contract["version"].as_str() != Some(version) {
            found.push("contract.json must name the same app and version as the Manifest".into());
        }
        if m.get("distribution").is_some() && !contract["signature"].is_string() {
            found.push(
                "a signed package (Manifest distribution) needs a signed contract.json".into(),
            );
        }
    }
    found
}

fn body(package: &Package) -> Value {
    let mut body = json!({"manifest": package.manifest});
    if let Some(contract) = &package.contract {
        body["contract"] = contract.clone();
    }
    body
}

async fn review(client: &Client, package: &Package) -> Outcome<Value> {
    Ok(client
        .apps()
        .post_apps_review(RequestOptions::json(body(package)))
        .await?)
}

fn print_review(run: &Run, review: &Value, extra: &[(&str, String)]) {
    let mut lines = vec![
        (
            "App",
            format!(
                "{} {}",
                output::cell(review.get("app")),
                output::cell(review.get("version"))
            ),
        ),
        ("Previous", output::cell(review.get("previousVersion"))),
        ("Digest", output::cell(review.get("digest"))),
        ("Contract", output::cell(review.get("contractDigest"))),
        ("Permissions", output::cell(review.get("permissions"))),
        ("Added", output::cell(review.get("added"))),
    ];
    lines.extend(extra.iter().cloned());
    output::fields(run.format, review, &lines);
}

/// `veyndo app …`
pub async fn run(run: &Run, command: AppCommand) -> Outcome {
    match command {
        AppCommand::New {
            template,
            directory,
            id,
        } => scaffold(run, template, directory, id),
        AppCommand::Validate {
            path,
            offline,
            sandbox,
        } => validate(run, &here(path), offline, sandbox).await,
        AppCommand::Deploy { path, sandbox } => deploy(run, &here(path), sandbox).await,
        AppCommand::Logs {
            app,
            sandbox,
            follow,
        } => logs(run, &app, sandbox, follow).await,
    }
}

fn scaffold(
    run: &Run,
    template: Option<String>,
    directory: Option<PathBuf>,
    id: Option<String>,
) -> Outcome {
    let Some(name) = template else {
        let rows: Vec<Value> = TEMPLATES
            .iter()
            .map(|t| json!({"name": t.name, "summary": t.summary, "tutorial": t.tutorial}))
            .collect();
        output::rows(
            run.format,
            &json!({"templates": rows}),
            &rows,
            &[
                column("TEMPLATE", &["/name"]),
                column("BUILDS", &["/summary"]),
            ],
        );
        return Ok(());
    };
    let template = templates::find(&name).ok_or_else(|| {
        let names: Vec<_> = TEMPLATES.iter().map(|t| t.name).collect();
        Failure::usage(format!(
            "Unknown template {name:?}; choose one of {}",
            names.join(", ")
        ))
    })?;
    let id = match (&id, &directory) {
        (Some(id), _) => id.clone(),
        (None, Some(dir)) => dir
            .file_name()
            .map(|n| n.to_string_lossy().replace('-', "_"))
            .unwrap_or_default(),
        (None, None) => format!("my_{}", template.name),
    };
    if !templates::valid_id(&id) {
        return Err(Failure::usage(format!(
            "App ID {id:?} must be lowercase letters, digits or underscores; pass --id"
        )));
    }
    let directory = directory.unwrap_or_else(|| PathBuf::from(&id));
    if directory.join("manifest.json").exists() {
        return Err(Failure::new(
            Exit::Conflict,
            format!("{} already contains a manifest.json", directory.display()),
        ));
    }
    std::fs::create_dir_all(&directory)?;
    let manifest = templates::manifest(template, &id);
    std::fs::write(
        directory.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap_or_default() + "\n",
    )?;
    std::fs::write(
        directory.join("README.md"),
        templates::readme(template, &id),
    )?;
    if run.json() {
        output::json(&json!({"app": id, "template": template.name, "directory": directory}));
    } else {
        println!(
            "Created {id} from the {} template in {}",
            template.name,
            directory.display()
        );
        output::note(&format!(
            "Next: `veyndo app validate {0}` and `veyndo app deploy {0}`",
            directory.display()
        ));
    }
    Ok(())
}

async fn validate(run: &Run, path: &Path, offline: bool, sandbox: Option<String>) -> Outcome {
    let package = load(path)?;
    let found = findings(&package);
    if !found.is_empty() {
        if run.json() {
            output::json(&json!({"valid": false, "findings": found}));
        } else {
            for finding in &found {
                println!("✗ {finding}");
            }
        }
        return Err(Failure::new(
            Exit::Findings,
            format!("{} finding(s) in {}", found.len(), package.root.display()),
        ));
    }
    if offline {
        if run.json() {
            output::json(&json!({"valid": true, "reviewed": false}));
        } else {
            println!("Files are well-formed; run without --offline for the server review.");
        }
        return Ok(());
    }
    let client = match &sandbox {
        Some(sandbox) => run.context.client_for(sandbox)?,
        None => run.context.client()?,
    };
    let reviewed = review(&client, &package)
        .await
        .map_err(|failure| match failure.exit {
            Exit::Invalid | Exit::Conflict => Failure::new(Exit::Findings, failure.message),
            _ => failure,
        })?;
    print_review(
        run,
        &reviewed,
        &[("Result", "valid; nothing was installed".into())],
    );
    Ok(())
}

async fn deploy(run: &Run, path: &Path, sandbox: Option<String>) -> Outcome {
    let package = load(path)?;
    let found = findings(&package);
    if let Some(first) = found.first() {
        return Err(Failure::new(
            Exit::Findings,
            format!("{first}; run `veyndo app validate`"),
        ));
    }
    let sandbox = match sandbox {
        Some(sandbox) => {
            sandbox::known(run, &sandbox).await?;
            sandbox
        }
        None => {
            let name = format!(
                "App {} {}",
                output::cell(package.manifest.get("id")),
                output::cell(package.manifest.get("version"))
            );
            let id = sandbox::create(run, &name).await?;
            output::note(&format!("Created sandbox {id}"));
            id
        }
    };
    let client = run.context.client_for(&sandbox)?;
    let reviewed = review(&client, &package).await?;
    let mut install = body(&package);
    install["approve"] = json!(true);
    install["digest"] = reviewed["digest"].clone();
    install["permissions"] = reviewed["permissions"].clone();
    if !reviewed["contractDigest"].is_null() {
        install["contractDigest"] = reviewed["contractDigest"].clone();
    }
    let installed = client
        .apps()
        .post_apps(RequestOptions::json(install))
        .await?;
    let summary = json!({"sandbox": sandbox, "review": reviewed, "installed": installed});
    if run.json() {
        output::json(&summary);
    } else {
        print_review(
            run,
            &reviewed,
            &[
                ("Sandbox", sandbox.clone()),
                ("Result", "installed in the sandbox".into()),
            ],
        );
        sandbox::next_steps(&sandbox);
    }
    Ok(())
}

/// Calls and deliveries in one list, newest last.
fn entries(activity: &Value) -> Vec<Value> {
    let mut rows = Vec::new();
    for call in activity["calls"].as_array().into_iter().flatten() {
        rows.push(json!({"at": call["at"], "kind": "call", "what": call["action"], "status": call["status"], "detail": call["durationMs"].as_i64().map(|ms| format!("{ms} ms")), "key": format!("call:{}", output::cell(call.get("id")))}));
    }
    for delivery in activity["deliveries"].as_array().into_iter().flatten() {
        rows.push(json!({"at": delivery["lastAttemptAt"], "kind": "delivery", "what": delivery["kind"], "status": delivery["state"], "detail": delivery["error"], "key": format!("delivery:{}:{}", output::cell(delivery.get("eventId")), output::cell(delivery.get("attempts")))}));
    }
    rows.sort_by_key(|row| output::cell(row.get("at")));
    rows
}

async fn logs(run: &Run, app: &str, sandbox: Option<String>, follow: bool) -> Outcome {
    let client = match &sandbox {
        Some(sandbox) => run.context.client_for(sandbox)?,
        None => run.context.client()?,
    };
    let columns = [
        column("AT", &["/at"]),
        column("KIND", &["/kind"]),
        column("WHAT", &["/what"]),
        column("STATUS", &["/status"]),
        column("DETAIL", &["/detail"]),
    ];
    let activity = client
        .apps()
        .get_apps_by_id_activity(app, RequestOptions::default())
        .await?;
    let rows = entries(&activity);
    if !follow {
        output::rows(run.format, &activity, &rows, &columns);
        return Ok(());
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut first = true;
    let mut pending = rows;
    loop {
        let fresh: Vec<Value> = pending
            .into_iter()
            .filter(|r| seen.insert(output::cell(r.get("key"))))
            .collect();
        if run.json() {
            for row in &fresh {
                println!("{row}");
            }
        } else if !fresh.is_empty() {
            let text = output::table(&fresh, &columns);
            let body = if first {
                text.as_str()
            } else {
                text.split_once('\n').map_or("", |(_, rest)| rest)
            };
            println!("{body}");
        }
        first = false;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let activity = client
            .apps()
            .get_apps_by_id_activity(app, RequestOptions::default())
            .await?;
        pending = entries(&activity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(manifest: Value, contract: Option<Value>) -> Package {
        Package {
            manifest,
            contract,
            root: PathBuf::from("."),
        }
    }

    #[test]
    fn local_findings_catch_file_mistakes() {
        let good = json!({"id":"my_app","version":"1.0.0","name":{"en":"My app"},"permissions":["data.read"]});
        assert!(findings(&package(good.clone(), None)).is_empty());
        let bad = json!({"id":"My-App","version":"1","name":{},"permissions":["a","a"]});
        assert_eq!(findings(&package(bad, None)).len(), 4);
        let mismatch = json!({"app":"other","version":"1.0.0"});
        assert_eq!(findings(&package(good.clone(), Some(mismatch))).len(), 1);
        let mut signed = good;
        signed["distribution"] = json!({"publisher":"acme"});
        let unsigned = json!({"app":"my_app","version":"1.0.0"});
        assert_eq!(findings(&package(signed, Some(unsigned))).len(), 1);
    }

    #[test]
    fn semantic_versions() {
        assert!(semver("1.2.3"));
        assert!(semver("1.2.3-beta.1"));
        assert!(!semver("1.2"));
        assert!(!semver("v1.2.3"));
    }

    #[test]
    fn activity_merges_calls_and_deliveries_in_time_order() {
        let activity = json!({
            "calls":[{"id":2,"action":"rules","status":"ok","durationMs":4,"at":"2026-11-28T10:00:02Z"}],
            "deliveries":[{"eventId":9,"kind":"order.placed","state":"delivered","attempts":1,"lastAttemptAt":"2026-11-28T10:00:01Z"}]
        });
        let rows = entries(&activity);
        assert_eq!(rows[0]["kind"], "delivery");
        assert_eq!(rows[1]["detail"], "4 ms");
    }
}
