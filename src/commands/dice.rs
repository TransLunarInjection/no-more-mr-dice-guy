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
#[description(r#"Rolls a dice.

1d20 roles a single d20.
2d10 roles two d10s.
(3d3 * 2) + 1d10 roles 3d3s, doubles them, then adds a d10.
10d20<15 roles 10d20 then filters only the rolls <15.
10d20! uses exploding dice. A roll of max value (20 in this case) will cause another roll to be made.
10d20!!>20 uses compounding exploding dice, and requires each roll to be >20. A roll of max value will cause another roll to be made and adds it to that dice, rather than treating it as a separate dice. It's possible for this roll to have a non-zero result.
Combine these options as you wish. It should hopefully work.
"#)]
#[usage("5d20")]
async fn roll(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
	let arg = args.message();
	let arg = if arg.is_empty() { "1d20" } else { arg };
	let result = crate::rolls::roll_expression(arg)?;
	msg.channel_id.say(&ctx.http, result).await?;
	Ok(())
}

#[command]
#[aliases(i)]
#[description("Inline rolls in a longer message. Repeats your message back to you with rolls in [[brackets]] replaced with the result of the roll.")]
#[usage("I attack the dragon [[2d20>15]].")]
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
