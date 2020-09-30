#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc)]
#![deny(clippy::unwrap_used)]
#![forbid(unsafe_code)]

#[macro_use]
extern crate lazy_static;

use anyhow::Result;
use serenity::client::bridge::gateway::{GatewayIntents, ShardManager};
use serenity::framework::StandardFramework;
use serenity::model::prelude::Activity;
use serenity::model::user::OnlineStatus;
use serenity::{async_trait, model::gateway::Ready, model::prelude::*, prelude::*};
use std::env;
use std::sync::Arc;

//top-level commands
pub mod commands;
pub mod rolls;
pub mod store;

pub mod prelude {
	pub use log::{error, info, warn};
}

use prelude::*;

const COLOR: (u8, u8, u8) = (186, 155, 255);
const CMD_PREFIX: &str = "d;";

struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
	type Value = Arc<Mutex<ShardManager>>;
}

struct RoleData;

impl TypeMapKey for RoleData {
	type Value = Arc<commands::roles::Persistent>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
	async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
		match commands::roles::handle_reaction(&ctx, &reaction, true).await {
			Ok(_) => {}
			Err(err) => {
				error!("Error handling reaction_add {:?}", err);
				let _ = send_warning_message(&ctx, reaction.channel_id, &format!("{}", err)).await;
			}
		}
	}

	async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
		match commands::roles::handle_reaction(&ctx, &reaction, false).await {
			Ok(_) => {}
			Err(err) => {
				error!("Error handling reaction_remove {:?}", err);
				let _ = send_warning_message(&ctx, reaction.channel_id, &format!("{}", err)).await;
			}
		}
	}

	async fn ready(&self, ctx: Context, ready: Ready) {
		let activity = Activity::playing(&format!("{}help | shard {}", CMD_PREFIX, ctx.shard_id));
		ctx.set_presence(Some(activity), OnlineStatus::Online).await;
		info!(
			"{} shard {} is connected to {} guilds",
			ready.user.name,
			ctx.shard_id,
			ready.guilds.len()
		);
	}
}

#[tokio::main]
async fn main() {
	if std::env::var_os("RUST_LOG") == None {
		std::env::set_var("RUST_LOG", "info");
	}

	tracing_subscriber::fmt::init();

	match start().await {
		Ok(_) => {}
		Err(e) => error!("{:?}", e),
	}
}

async fn start() -> Result<()> {
	let intents = GatewayIntents::GUILDS
		| GatewayIntents::DIRECT_MESSAGES // DM commands
		| GatewayIntents::GUILD_EMOJIS // emoji
		| GatewayIntents::GUILD_MESSAGE_REACTIONS // guild role reacts
		| GatewayIntents::GUILD_MESSAGES; // guild commands

	// Configure the client with your Discord bot token in the environment.
	let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

	let framework = commands::register(StandardFramework::new().configure(|c| {
		c.prefix(CMD_PREFIX)
			.allow_dm(true)
			.case_insensitivity(true)
			.ignore_bots(false)
			.ignore_webhooks(false)
			.delimiters(vec![" "])
	}));

	let mut client = Client::new(&token)
		.intents(intents)
		.event_handler(Handler)
		.framework(framework)
		.await
		.expect("Err creating client");

	{
		let mut data = client.data.write().await;
		data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
		data.insert::<RoleData>(Arc::new(commands::roles::Persistent::default()));
	}

	{
		let manager = client.shard_manager.clone();
		ctrlc::set_handler(move || {
			stop_client(&*manager);
		})
		.expect("Failed to set ctrlc handler");
	}

	// Automatically picks shard count per discord API's suggestion
	match client.start_autosharded().await {
		Ok(_) => {
			info!("Client stopped normally");
		}
		Err(err) => {
			error!("Client error: {:?}", err);
		}
	}

	Ok(())
}

#[tokio::main]
async fn stop_client(manager: &Mutex<ShardManager>) {
	let mut manager = manager.lock().await;
	manager.shutdown_all().await;
}

async fn send_warning_message(ctx: &Context, channel: ChannelId, text: &str) -> anyhow::Result<()> {
	channel
		.send_message(&ctx.http, |m| {
			warning_message(m, text);
			m
		})
		.await?;

	Ok(())
}

fn warning_message(m: &mut serenity::builder::CreateMessage, msg: &str) {
	m.embed(|e| {
		e.title("Error");

		// warning triangle emoji
		e.description(format!("\u{26a0}\u{fe0f}{}", msg));

		e
	});
}
