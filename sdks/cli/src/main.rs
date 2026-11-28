//! `veyndo` command-line entry: argument grammar, help text and dispatch; every call goes through the generated Rust SDK.
mod account;
mod agents;
mod apps;
mod catalog;
mod failure;
mod output;
mod profile;
mod sandbox;
mod templates;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use failure::{Exit, Outcome};
use output::Format;
use profile::{Context, Selection};
use std::path::PathBuf;
use std::process::ExitCode;

const AFTER_HELP: &str = "\
Examples:
  veyndo login --email you@example.com
  veyndo shop use my-shop
  veyndo products import catalog.csv
  veyndo sandbox release <sandbox> --yes
  veyndo app new advisor my_advisor && veyndo app deploy my_advisor
  veyndo ops check plan.json

This is the merchant and developer client (crate veyndo-cli). The server is a different program
built from the repository root (`/app/veyndo` in the production image); operator flags such as
--provision or --sign-app belong to the server, never to this client.
Exit codes: 0 ok, 1 failure, 2 usage, 3 not signed in, 4 permission, 5 not found,
6 invalid input, 7 conflict, 8 unavailable, 10 check findings. See docs/cli.md.";

/// Manage Veyndo shops, catalogues, apps and agents from the terminal.
#[derive(Parser, Debug)]
#[command(name = "veyndo", version, about, after_help = AFTER_HELP, propagate_version = true)]
struct Cli {
    #[command(flatten)]
    global: Global,
    #[command(subcommand)]
    command: Command,
}

/// Options every command accepts.
#[derive(Args, Debug, Clone)]
struct Global {
    /// Server origin (default: the stored profile, else https://api.veyndo.ai)
    #[arg(long, global = true, env = "VEYNDO_SERVER", value_name = "URL")]
    server: Option<String>,
    /// Shop ID for this call (default: the shop chosen with `veyndo shop use`)
    #[arg(long, global = true, env = "VEYNDO_SHOP", value_name = "SHOP")]
    shop: Option<String>,
    /// Print JSON instead of tables
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Sign in with a personal account or an integration key
    Login(account::Login),
    /// Forget the stored credential (and end a personal session on the server)
    Logout,
    /// Show server, account, shop and permissions
    Status,
    /// List, select or create shops
    #[command(subcommand)]
    Shop(account::ShopCommand),
    /// Read products and import a CSV into a private sandbox
    #[command(subcommand)]
    Products(catalog::ProductsCommand),
    /// Read orders
    #[command(subcommand)]
    Orders(catalog::OrdersCommand),
    /// Private sandboxes: list, create, review changes and release them to the live shop
    #[command(subcommand)]
    Sandbox(sandbox::SandboxCommand),
    /// Scaffold, validate, deploy and observe apps
    #[command(subcommand)]
    App(apps::AppCommand),
    /// The typed operations graph and the plan checker
    #[command(subcommand)]
    Ops(agents::OpsCommand),
    /// Agent tooling
    #[command(subcommand)]
    Agent(agents::AgentCommand),
    /// Print the MCP endpoint and client configuration, or bridge MCP over stdio
    Mcp(agents::Mcp),
    /// Print a shell completion script
    Completions {
        /// Target shell
        #[arg(value_enum)]
        shell: Shell,
    },
}

/// Shells with completion scripts.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    Fish,
    Elvish,
    Powershell,
}

/// What every command receives: the resolved profile and the output format.
pub struct Run {
    /// Effective server, shop and credential.
    pub context: Context,
    /// Table or JSON.
    pub format: Format,
}

impl Run {
    /// `true` in JSON mode.
    pub fn json(&self) -> bool {
        self.format == Format::Json
    }
}

/// A path argument that defaults to the current directory.
pub fn here(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(|| PathBuf::from("."))
}

fn completions(shell: Shell) {
    let mut command = Cli::command();
    let out = &mut std::io::stdout();
    match shell {
        Shell::Bash => {
            clap_complete::generate(clap_complete::Shell::Bash, &mut command, "veyndo", out)
        }
        Shell::Zsh => {
            clap_complete::generate(clap_complete::Shell::Zsh, &mut command, "veyndo", out)
        }
        Shell::Fish => {
            clap_complete::generate(clap_complete::Shell::Fish, &mut command, "veyndo", out)
        }
        Shell::Elvish => {
            clap_complete::generate(clap_complete::Shell::Elvish, &mut command, "veyndo", out);
        }
        Shell::Powershell => {
            clap_complete::generate(
                clap_complete::Shell::PowerShell,
                &mut command,
                "veyndo",
                out,
            );
        }
    }
}

async fn dispatch(cli: Cli) -> Outcome {
    if let Command::Completions { shell } = cli.command {
        completions(shell);
        return Ok(());
    }
    let selection = Selection {
        server: cli.global.server.clone(),
        shop: cli.global.shop.clone(),
    };
    let run = Run {
        context: Context::resolve(&selection)?,
        format: if cli.global.json {
            Format::Json
        } else {
            Format::Table
        },
    };
    match cli.command {
        Command::Login(login) => account::login(&run, login).await,
        Command::Logout => account::logout(&run).await,
        Command::Status => account::status(&run).await,
        Command::Shop(command) => account::shop(&run, command).await,
        Command::Products(command) => catalog::products(&run, command).await,
        Command::Orders(command) => catalog::orders(&run, command).await,
        Command::Sandbox(command) => sandbox::run(&run, command).await,
        Command::App(command) => apps::run(&run, command).await,
        Command::Ops(command) => agents::ops(&run, command).await,
        Command::Agent(command) => agents::agent(&run, command),
        Command::Mcp(mcp) => agents::mcp(&run, mcp).await,
        Command::Completions { .. } => Ok(()),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("veyndo: {error}");
            return ExitCode::from(Exit::Failure as u8);
        }
    };
    match runtime.block_on(dispatch(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("veyndo: {failure}");
            ExitCode::from(failure.exit as u8)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_grammar_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn subcommand_arguments_never_shadow_the_global_shop() {
        let cli = Cli::try_parse_from(["veyndo", "shop", "use", "other"]).unwrap();
        assert_eq!(cli.global.shop, None);
        let cli = Cli::try_parse_from(["veyndo", "--shop", "a", "sandbox", "diff", "s1"]).unwrap();
        assert_eq!(cli.global.shop.as_deref(), Some("a"));
    }

    #[test]
    fn completions_cover_every_command() {
        let mut command = Cli::command();
        let mut out = Vec::new();
        clap_complete::generate(clap_complete::Shell::Bash, &mut command, "veyndo", &mut out);
        let script = String::from_utf8(out).unwrap();
        for name in [
            "login",
            "products",
            "sandbox",
            "app",
            "ops",
            "mcp",
            "completions",
        ] {
            assert!(script.contains(name), "{name} missing from completions");
        }
    }
}
