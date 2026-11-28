//! Sign-in, sign-out, status and shop selection over the identity API; secrets are read without echo and never printed.
use crate::Run;
use crate::failure::{Exit, Failure, Outcome};
use crate::output::{self, column};
use crate::profile::{self, Credential, Kind, Profile};
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use std::io::{BufRead, IsTerminal};
use veyndo_sdk::{Client, Error, RequestOptions};

/// `veyndo login`
#[derive(Args, Debug)]
#[command(after_help = "\
The server offers no browser device sign-in yet, so the CLI signs in with one of:
  --email        a personal merchant account (password and authenticator code are prompted
                 without echo, or read from stdin with --password-stdin); the session lasts 12 hours
  --with-key     an integration key from Studio → Developer → API & integrations, read from
                 stdin or a hidden prompt; it is bound to one shop, so pass --shop
The credential is stored in an owner-only file (0600), see `veyndo status`.
For CI, set VEYNDO_API_KEY and VEYNDO_SHOP instead of signing in.")]
pub struct Login {
    /// Personal account email
    #[arg(long, value_name = "EMAIL", conflicts_with = "with_key")]
    email: Option<String>,
    /// Read the password from the first line of stdin instead of a prompt
    #[arg(long, requires = "email")]
    password_stdin: bool,
    /// Authenticator or recovery code, if the account uses two-factor sign-in
    #[arg(long, value_name = "CODE", requires = "email")]
    mfa_code: Option<String>,
    /// Sign in with an integration key (read from stdin or a hidden prompt, never an argument)
    #[arg(long)]
    with_key: bool,
}

/// `veyndo shop …`
#[derive(Subcommand, Debug)]
pub enum ShopCommand {
    /// List the shops your account belongs to
    List,
    /// Select the shop later commands use
    Use {
        /// Shop ID
        #[arg(value_name = "SHOP")]
        id: String,
    },
    /// Create a new shop (personal account only)
    Create {
        /// Shop ID: lowercase letters, digits and hyphens
        id: String,
        /// Display name
        #[arg(long)]
        name: String,
    },
}

/// Reads a secret: the first stdin line when piped, else a prompt without echo.
fn secret(prompt: &str) -> Outcome<String> {
    let value = if std::io::stdin().is_terminal() {
        rpassword::prompt_password(prompt)?
    } else {
        first_line()?
    };
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(Failure::invalid("No secret given"));
    }
    Ok(value)
}

fn first_line() -> Outcome<String> {
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(line)
}

fn prompt(text: &str) -> Outcome<String> {
    if !std::io::stdin().is_terminal() {
        return Err(Failure::usage(format!(
            "{} needs a terminal; pass it as an option",
            text.trim_end_matches(": ")
        )));
    }
    eprint!("{text}");
    Ok(first_line()?.trim().to_string())
}

fn stored(run: &Run, shop: Option<String>, credential: Credential) -> Profile {
    Profile {
        server: Some(run.context.server.clone()),
        shop,
        credential: Some(credential),
    }
}

/// `veyndo login`
pub async fn login(run: &Run, login: Login) -> Outcome {
    if login.with_key {
        return login_key(run).await;
    }
    let email = match login.email {
        Some(email) => email,
        None => prompt("Email: ")?,
    };
    let password = if login.password_stdin {
        first_line()?.trim_end_matches(['\r', '\n']).to_string()
    } else {
        secret("Password: ")?
    };
    let anonymous = run.context.anonymous();
    let mut session = anonymous
        .identity()
        .post_auth_login(RequestOptions::json(
            json!({"email": email, "password": password}),
        ))
        .await?;
    if session["mfaRequired"] == json!(true) {
        let code = match login.mfa_code {
            Some(code) => code,
            None => secret("Authenticator or recovery code: ")?,
        };
        session = anonymous
            .identity()
            .post_auth_mfa_verify(RequestOptions::json(json!({
                "challengeToken": session["challengeToken"],
                "code": code,
            })))
            .await?;
    }
    let token = session["token"]
        .as_str()
        .ok_or_else(|| Failure::new(Exit::Failure, "The server returned no session"))?;
    let shop = run
        .context
        .shop
        .clone()
        .or_else(|| session["workspace"].as_str().map(String::from));
    let credential = Credential {
        kind: Kind::Session,
        secret: token.to_string(),
        email: Some(email.clone()),
    };
    let path = profile::save(&stored(run, shop.clone(), credential))?;
    let summary = json!({"signedIn": true, "kind": "session", "email": email, "shop": shop, "server": run.context.server});
    report(run, &summary, &path);
    Ok(())
}

async fn login_key(run: &Run) -> Outcome {
    let shop = run.context.require_shop().map_err(|_| {
        Failure::usage("An integration key is bound to one shop: pass --shop <id> (or VEYNDO_SHOP)")
    })?;
    let key = secret("Integration key: ")?;
    let credential = Credential {
        kind: Kind::IntegrationKey,
        secret: key,
        email: None,
    };
    let mut context = run.context.clone();
    context.credential = Some(credential.clone());
    let access = context
        .client_for(&shop)?
        .identity()
        .get_auth_access(RequestOptions::default())
        .await?;
    let path = profile::save(&stored(run, Some(shop.clone()), credential))?;
    let summary = json!({"signedIn": true, "kind": "integration-key", "shop": shop, "server": run.context.server, "permissions": access["permissions"]});
    report(run, &summary, &path);
    Ok(())
}

fn report(run: &Run, summary: &Value, path: &std::path::Path) {
    if run.json() {
        output::json(summary);
        return;
    }
    let who = summary["email"].as_str().map_or_else(
        || "an integration key".to_string(),
        |email| format!("as {email}"),
    );
    println!(
        "Signed in {who} on {}{}",
        run.context.server,
        summary["shop"]
            .as_str()
            .map(|s| format!(", shop {s}"))
            .unwrap_or_default()
    );
    output::note(&format!(
        "Credential stored in {} (owner-only)",
        path.display()
    ));
}

/// `veyndo logout`
pub async fn logout(run: &Run) -> Outcome {
    let stored = profile::load()?;
    let mut ended = false;
    if let Some(credential) = stored
        .credential
        .as_ref()
        .filter(|c| c.kind == Kind::Session)
    {
        let mut context = run.context.clone();
        context.credential = Some(credential.clone());
        let shop = context.shop.clone().unwrap_or_default();
        // An expired session is already gone on the server; forgetting it locally is the point.
        ended = context
            .client_for(&shop)?
            .identity()
            .post_auth_logout(RequestOptions::json(json!({})))
            .await
            .is_ok();
    }
    let removed = profile::remove()?;
    if run.json() {
        output::json(&json!({"removed": removed, "sessionEnded": ended}));
    } else if removed {
        println!(
            "Signed out{}.",
            if ended { " and ended the session" } else { "" }
        );
    } else {
        println!("No stored credential.");
    }
    if run.context.from_environment {
        output::note("VEYNDO_API_KEY is still set in this environment.");
    }
    Ok(())
}

/// `veyndo status`: never fails on a missing credential, so it is safe in scripts and prompts.
pub async fn status(run: &Run) -> Outcome {
    let context = &run.context;
    let health = context
        .anonymous()
        .host()
        .get_health(RequestOptions::default())
        .await;
    let reachable = health.as_ref().is_ok_and(|h| h["status"] == "ok");
    let kind = context.credential.as_ref().map(|c| c.kind);
    let mut summary = json!({
        "server": context.server, "reachable": reachable, "shop": context.shop,
        "signedIn": false, "credential": kind.map(|k| match k { Kind::Session => "session", Kind::IntegrationKey => "integration-key" }),
        "credentialSource": if context.from_environment { "VEYNDO_API_KEY" } else { "profile" },
        "profile": profile::directory()?.join("credentials.json"),
    });
    if let (true, Some(shop)) = (reachable && kind.is_some(), context.shop.as_deref()) {
        let client = context.client_for(shop)?;
        match client
            .identity()
            .get_auth_access(RequestOptions::default())
            .await
        {
            Ok(access) => {
                summary["signedIn"] = json!(true);
                summary["permissions"] = access["permissions"].clone();
                summary["personalSession"] = access["personalSession"].clone();
            }
            Err(error) => summary["problem"] = json!(Failure::from(error).message),
        }
        if kind == Some(Kind::Session)
            && let Ok(session) = client
                .identity()
                .get_auth_session(RequestOptions::default())
                .await
        {
            summary["email"] = session["user"]["email"].clone();
        }
    }
    let line = |key: &str| output::cell(summary.get(key));
    let permissions = summary["permissions"].as_array().map_or(0, Vec::len);
    output::fields(
        run.format,
        &summary,
        &[
            (
                "Server",
                format!(
                    "{} ({})",
                    line("server"),
                    if reachable {
                        "reachable"
                    } else {
                        "unreachable"
                    }
                ),
            ),
            ("Shop", line("shop")),
            (
                "Signed in",
                if summary["signedIn"] == json!(true) {
                    "yes".into()
                } else {
                    "no".into()
                },
            ),
            (
                "Credential",
                format!("{} from {}", line("credential"), line("credentialSource")),
            ),
            ("Account", line("email")),
            ("Permissions", format!("{permissions} granted")),
            ("Problem", line("problem")),
            ("Profile", line("profile")),
        ],
    );
    Ok(())
}

async fn session_workspaces(run: &Run) -> Outcome<(Client, Value)> {
    require_session(run, "Listing shops")?;
    let shop = run.context.shop.clone().unwrap_or_default();
    let client = run.context.client_for(&shop)?;
    let session = client
        .identity()
        .get_auth_session(RequestOptions::default())
        .await?;
    Ok((client, session))
}

fn require_session(run: &Run, what: &str) -> Outcome {
    match run.context.credential.as_ref().map(|c| c.kind) {
        Some(Kind::Session) => Ok(()),
        Some(Kind::IntegrationKey) => Err(Failure::new(
            Exit::Permission,
            format!(
                "{what} needs a personal sign-in (`veyndo login --email`); an integration key is bound to one shop"
            ),
        )),
        None => Err(Failure::signed_out("Not signed in; run `veyndo login`")),
    }
}

/// `veyndo shop …`
pub async fn shop(run: &Run, command: ShopCommand) -> Outcome {
    match command {
        ShopCommand::List => {
            let (_, session) = session_workspaces(run).await?;
            let mut rows = session["workspaces"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for row in &mut rows {
                row["selected"] = json!(row["id"].as_str() == run.context.shop.as_deref());
            }
            output::rows(
                run.format,
                &json!({"shops": rows}),
                &rows,
                &[
                    column("ID", &["/id"]),
                    column("NAME", &["/name"]),
                    column("ROLE", &["/role"]),
                    column("SELECTED", &["/selected"]),
                ],
            );
            Ok(())
        }
        ShopCommand::Use { id } => use_shop(run, id).await,
        ShopCommand::Create { id, name } => create_shop(run, id, name).await,
    }
}

async fn use_shop(run: &Run, shop: String) -> Outcome {
    let mut stored = profile::load()?;
    match run.context.credential.as_ref().map(|c| c.kind) {
        Some(Kind::Session) => {
            let (_, session) = session_workspaces(run).await?;
            let member = session["workspaces"]
                .as_array()
                .is_some_and(|all| all.iter().any(|w| w["id"] == json!(shop)));
            if !member {
                return Err(Failure::new(
                    Exit::NotFound,
                    format!("Your account has no shop {shop:?}; see `veyndo shop list`"),
                ));
            }
        }
        Some(Kind::IntegrationKey) if stored.shop.as_deref() != Some(shop.as_str()) => {
            return Err(Failure::new(
                Exit::Permission,
                "An integration key is bound to the shop it was created for; sign in again for another shop",
            ));
        }
        _ => {}
    }
    stored.shop = Some(shop.clone());
    stored.server.get_or_insert(run.context.server.clone());
    profile::save(&stored)?;
    if run.json() {
        output::json(&json!({"shop": shop}));
    } else {
        println!("Using shop {shop}");
    }
    Ok(())
}

async fn create_shop(run: &Run, id: String, name: String) -> Outcome {
    let (client, _) = session_workspaces(run).await?;
    let created = client
        .identity()
        .post_workspaces(RequestOptions::json(
            json!({"workspaceId": id, "workspaceName": name}),
        ))
        .await
        .map_err(|error| match error {
            Error::Api(api) if api.status == 403 => Failure::new(
                Exit::Permission,
                format!(
                    "This account may not create shops here: {}",
                    api.problems.first().map_or("", |p| p.detail.as_str())
                ),
            ),
            other => other.into(),
        })?;
    // Creation returns a fresh personal session for the new shop; keep using it.
    let mut stored = profile::load()?;
    if let (Some(token), Some(credential)) = (created["token"].as_str(), stored.credential.as_mut())
    {
        credential.secret = token.to_string();
    }
    stored.shop = Some(created["workspace"].as_str().unwrap_or(&id).to_string());
    profile::save(&stored)?;
    if run.json() {
        output::json(&json!({"shop": stored.shop, "created": true}));
    } else {
        println!("Created shop {id} ({name}) and selected it");
    }
    Ok(())
}
