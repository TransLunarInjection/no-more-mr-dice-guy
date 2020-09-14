#[macro_use]
extern crate lazy_static;

use anyhow::Result;
use regex::{Captures, Regex};
use serenity::model::prelude::Activity;
use serenity::model::user::OnlineStatus;
use serenity::{
	async_trait,
	model::{channel::Message, gateway::Ready},
	prelude::*,
};
use std::env;

pub mod rolls;

struct Handler;

const CMD_PREFIX: &str = "d;";

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
			msg.channel_id.say(&ctx.http, result?).await
		}
		"inline" | "i" => {
			let result = inline_rolls(&msg, cmd_parts.collect::<Vec<&str>>().join(" ")).await;
			msg.channel_id.say(&ctx.http, result?).await
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

async fn inline_rolls(msg: &Message, message: String) -> Result<String> {
	lazy_static! {
		static ref ROLL_REGEX: Regex = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
	}
	let nick = &msg.author.name;
	let mut err = None;
	let rolled = ROLL_REGEX.replace_all(&message, |caps: &Captures| {
		match rolls::roll_expression(&caps[1]) {
			Ok(rolled) => rolled,
			Err(e) => {
				err = Some(e);
				"".to_string()
			}
		}
	});
	match err {
		Some(err) => Err(err),
		_ => Ok(format!("{}: {}", nick, rolled)),
	}
}

fn roll<'a>(cmd_parts: impl Iterator<Item = &'a str>) -> Result<String> {
	let parts: Vec<&str> = cmd_parts.collect();
	Ok(if parts.is_empty() {
		rolls::roll_expression("1d20")?
	} else {
		rolls::roll_expression(&parts.join(" "))?
	})
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
