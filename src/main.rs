use anyhow::Result;
use clap::{CommandFactory, Parser};
use std::io;
use std::io::Write;

mod cli;
mod commands;
mod context;
mod download;
mod helpers;
mod resolve;

use cli::{Cli, Commands, Shell};
use context::AppContext;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let ctx = AppContext {
        quiet: cli.quiet,
        verbose: cli.verbose,
        dry_run: cli.dry_run,
    };

    let result = match cli.command {
        Commands::Auth(cmd) => commands::auth::handle(&ctx, cmd).await,
        Commands::Space(cmd) => commands::space::handle(&ctx, cmd).await,
        Commands::Page(cmd) => commands::page::handle(&ctx, cmd).await,
        Commands::Search(cmd) => commands::search::handle(&ctx, cmd).await,
        Commands::Attachment(cmd) => commands::attachment::handle(&ctx, cmd).await,
        Commands::Label(cmd) => commands::label::handle(&ctx, cmd).await,
        Commands::Comment(cmd) => commands::comment::handle(&ctx, cmd).await,
        Commands::Export(args) => commands::export::handle(&ctx, args).await,
        #[cfg(feature = "write")]
        Commands::CopyTree(args) => commands::copy_tree::handle(&ctx, args).await,
        Commands::Completions(args) => generate_completions(args),
    };

    if let Err(err) = result {
        if ctx.verbose > 0 {
            eprintln!("{err:?}");
        } else {
            eprintln!("{}", format_error_chain(&err));
        }
        std::process::exit(1);
    }

    Ok(())
}

fn format_error_chain(err: &anyhow::Error) -> String {
    let mut out = err.to_string();
    for cause in err.chain().skip(1) {
        out.push_str(": ");
        out.push_str(&cause.to_string());
    }
    out
}

fn generate_completions(args: cli::CompletionsArgs) -> Result<()> {
    let mut cmd = Cli::command();
    let shell = match args.shell {
        Shell::Bash => clap_complete::Shell::Bash,
        Shell::Zsh => clap_complete::Shell::Zsh,
        Shell::Fish => clap_complete::Shell::Fish,
        Shell::Pwsh => clap_complete::Shell::PowerShell,
    };

    // `clap_complete::generate(..., &mut stdout())` can panic on broken pipes
    // (e.g. `confcli completions bash | head`). Generate into a buffer first,
    // then write it to stdout and gracefully ignore BrokenPipe.
    let mut buf: Vec<u8> = Vec::new();
    clap_complete::generate(shell, &mut cmd, "confcli", &mut buf);

    let mut stdout = io::stdout().lock();
    if let Err(err) = stdout.write_all(&buf) {
        if err.kind() == io::ErrorKind::BrokenPipe {
            return Ok(());
        }
        return Err(err.into());
    }

    Ok(())
}
