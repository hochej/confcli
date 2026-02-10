use anyhow::Result;

use crate::cli::*;
use crate::context::AppContext;

mod listing;
mod navigation;
#[cfg(feature = "write")]
mod write_ops;

pub async fn handle(ctx: &AppContext, cmd: PageCommand) -> Result<()> {
    let client = crate::context::load_client(ctx)?;
    match cmd {
        PageCommand::List(args) => listing::page_list(&client, ctx, args).await,
        PageCommand::Get(args) => listing::page_get(&client, ctx, args).await,
        PageCommand::Body(args) => listing::page_body(&client, ctx, args).await,
        #[cfg(feature = "write")]
        PageCommand::Edit(args) => write_ops::page_edit(&client, ctx, args).await,
        #[cfg(feature = "write")]
        PageCommand::Create(args) => write_ops::page_create(&client, ctx, args).await,
        #[cfg(feature = "write")]
        PageCommand::Update(args) => write_ops::page_update(&client, ctx, args).await,
        #[cfg(feature = "write")]
        PageCommand::Delete(args) => write_ops::page_delete(&client, ctx, args).await,
        PageCommand::Children(args) => navigation::page_children(&client, ctx, args).await,
        PageCommand::History(args) => navigation::page_history(&client, ctx, args).await,
        PageCommand::Open(args) => navigation::page_open(&client, ctx, args).await,
    }
}
