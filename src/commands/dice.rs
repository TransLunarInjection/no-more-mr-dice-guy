use anyhow::Result;
use regex::{Captures, Regex};

use super::prelude::*;
use serenity::framework::standard::{Args, CommandResult};
use serenity::framework::StandardFramework;

pub fn register(framework: StandardFramework) -> StandardFramework {
	framework.group(&DICE_GROUP)
}

#[group]
#[commands(inline, roll)]
struct Dice;

#[command]
#[aliases(r)]
async fn roll(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
	let arg = args.message();
	let arg = if arg.is_empty() { "1d20" } else { arg };
	let result = crate::rolls::roll_expression(arg)?;
	msg.channel_id.say(&ctx.http, result).await?;
	Ok(())
}

#[command]
#[aliases(i)]
async fn inline(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
	let result = inline_rolls(msg, args.message()).await;
	msg.channel_id.say(&ctx.http, result?).await?;
	Ok(())
}

async fn inline_rolls(msg: &Message, message: &str) -> Result<String> {
	lazy_static! {
		static ref ROLL_REGEX: Regex = Regex::new(r"\[\[([^\]]+)\]\]").expect("Hardcoded regex");
	}
	let mut nick: &str = &msg.author.name;
	if let Some(idx) = nick.rfind('|') {
		nick = nick[0..idx].trim();
	}
	let mut err = None;
	let rolled =
		ROLL_REGEX.replace_all(
			message,
			|caps: &Captures| match crate::rolls::roll_expression(&caps[1]) {
				Ok(rolled) => rolled,
				Err(e) => {
					err = Some(e);
					"".to_string()
				}
			},
		);
	match err {
		Some(err) => Err(err),
		None => Ok(format!("{}: {}", nick, rolled)),
	}
}
