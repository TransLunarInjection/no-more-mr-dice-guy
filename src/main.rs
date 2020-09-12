#[macro_use]
extern crate lazy_static;

use anyhow::{Error, Result};
use rand::Rng;
use regex::{Captures, Regex};
use serenity::model::guild::PartialMember;
use serenity::model::prelude::{Activity, Member};
use serenity::model::user::OnlineStatus;
use serenity::{
	async_trait,
	model::{channel::Message, gateway::Ready},
	prelude::*,
};
use std::env;

pub mod rolls;

// Serenity implements transparent sharding in a way that you do not need to
// manually handle separate processes or connections manually.
//
// Transparent sharding is useful for a shared cache. Instead of having caches
// with duplicated data, a shared cache means all your data can be easily
// accessible across all shards.
//
// If your bot is on many guilds - or over the maximum of 2500 - then you
// should/must use guild sharding.
//
// This is an example file showing how guild sharding works. For this to
// properly be able to be seen in effect, your bot should be in at least 2
// guilds.
//
// Taking a scenario of 2 guilds, try saying "!ping" in one guild. It should
// print either "0" or "1" in the console. Saying "!ping" in the other guild,
// it should cache the other number in the console. This confirms that guild
// sharding works.
struct Handler;

const CMD_PREFIX: &'static str = "d;";

async fn handle_command(ctx: Context, msg: Message) -> Result<()> {
	let mut cmd_parts = msg.content.trim_start_matches(CMD_PREFIX).split(' ');
	let cmd = cmd_parts.next();
	let cmd = match cmd {
		None => {
			msg.channel_id
				.say(&ctx.http, "invalid command: no command specified")
				.await?;
			return Ok(());
		}
		Some(cmd) => cmd,
	};

	match cmd {
		"ping" => msg.channel_id.say(&ctx.http, "Pong!").await,
		"roll" | "r" => {
			let result = roll(cmd_parts);
			msg.channel_id.say(&ctx.http, result).await
		}
		"inline" | "i" => {
			let result = inline_rolls(&ctx, &msg, cmd_parts.collect::<Vec<&str>>().join(" ")).await;
			msg.channel_id.say(&ctx.http, result).await
		}
		_ => {
			msg.channel_id
				.say(
					&ctx.http,
					&format!("invalid command: unknown command {}", cmd),
				)
				.await
		}
	}?;

	Ok(())
}

async fn inline_rolls(ctx: &Context, msg: &Message, message: String) -> String {
	lazy_static! {
		static ref ROLL_REGEX: Regex = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
	}
	let nick = &msg.author.name;
	format!(
		"{}: {}",
		nick,
		ROLL_REGEX.replace_all(&message, |caps: &Captures| {
			format!("{}", rolls::roll_expression(&caps[1]))
		})
	)
}

fn roll<'a>(mut cmd_parts: impl Iterator<Item = &'a str>) -> String {
	let parts: Vec<&str> = cmd_parts.collect();
	if parts.is_empty() {
		return rolls::roll_expression("1d20");
	}
	return rolls::roll_expression(&parts.join(" "));
}

#[async_trait]
impl EventHandler for Handler {
	async fn message(&self, ctx: Context, msg: Message) {
		if msg.content.starts_with(CMD_PREFIX) {
			match handle_command(ctx, msg).await {
				Ok(_) => {}
				Err(e) => eprintln!("{:?}", e),
			}
		}
	}

	async fn ready(&self, ctx: Context, ready: Ready) {
		let activity = Activity::playing(&format!("rolling on shard {}", ctx.shard_id));
		ctx.set_presence(Some(activity), OnlineStatus::Online).await;
		println!("{} is connected!", ready.user.name);
	}
}

#[tokio::main]
async fn main() {
	// Configure the client with your Discord bot token in the environment.
	let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
	let mut client = Client::new(&token)
		.event_handler(Handler)
		.await
		.expect("Err creating client");

	// The total number of shards to use. The "current shard number" of a
	// shard - that is, the shard it is assigned to - is indexed at 0,
	// while the total shard count is indexed at 1.
	//
	// This means if you have 5 shards, your total shard count will be 5, while
	// each shard will be assigned numbers 0 through 4.
	if let Err(why) = client.start_shards(2).await {
		println!("Client error: {:?}", why);
	}
}
