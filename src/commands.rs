use serenity::client::bridge::gateway::ShardId;
use serenity::framework::standard::{
	help_commands, Args, CommandGroup, CommandResult, DispatchError, HelpOptions,
};
use serenity::framework::StandardFramework;
use std::collections::HashSet;

pub mod checks;
pub mod dice;
pub mod roles;

pub mod prelude {
	pub use super::checks::*;
	pub use crate::prelude::*;
	pub use serenity::{framework::standard::macros::*, model::prelude::*, prelude::*};
}

use prelude::*;

#[group]
#[commands(invite, latency)]
struct General;

#[command]
#[aliases(ping)]
#[description("Gets the current shard's latency")]
async fn latency(ctx: &Context, msg: &Message) -> CommandResult {
	// The shard manager is an interface for mutating, stopping, restarting, and
	// retrieving information about shards.
	let data = ctx.data.read().await;

	let shard_manager = if let Some(v) = data.get::<super::ShardManagerContainer>() {
		v
	} else {
		let _ = msg
			.reply(ctx, "There was a problem getting the shard manager")
			.await;

		return Ok(());
	};

	let manager = shard_manager.lock().await;
	let runners = manager.runners.lock().await;

	let runner = if let Some(runner) = runners.get(&ShardId(ctx.shard_id)) {
		runner
	} else {
		let _ = msg.reply(ctx, "No shard found");

		return Ok(());
	};

	let _ = msg
		.reply(ctx, &format!("The shard latency is {:?}", runner.latency))
		.await;

	Ok(())
}

#[command]
#[description("Gets this bot's invite link")]
async fn invite(ctx: &Context, msg: &Message) -> CommandResult {
	let id = ctx.cache.current_user_id().await;
	msg.reply(
		&ctx,
		format!(
			"<https://discord.com/oauth2/authorize?client_id={}&scope=bot&permissions=0>",
			id
		),
	)
	.await?;

	Ok(())
}

// The framework provides two built-in help commands for you to use.
// But you can also make your own customized help command that forwards
// to the behaviour of either of them.
#[help]
// This replaces the information that a user can pass
// a command-name as argument to gain specific information about it.
#[aliases_label(aliases)]
// Some arguments require a `{}` in order to replace it with contextual information.
// In this case our `{}` refers to a command's name.
#[command_not_found_text = "Could not find: `{}`."]
// Define the maximum Levenshtein-distance between a searched command-name
// and commands. If the distance is lower than or equal the set distance,
// it will be displayed as a suggestion.
// Setting the distance to 0 will disable suggestions.
#[max_levenshtein_distance(3)]
// When you use sub-groups, Serenity will use the `indention_prefix` to indicate
// how deeply an item is indented.
// The default value is "-", it will be changed to "+".
#[indention_prefix = "+"]
// On another note, you can set up the help-menu-filter-behaviour.
// Here are all possible settings shown on all possible options.
// First case is if a user lacks permissions for a command, we can hide the command.
#[lacking_permissions = "Strike"]
// If the user is nothing but lacking a certain role, we just display it hence our variant is `Nothing`.
#[lacking_role = "Strike"]
// The last `enum`-variant is `Strike`, which ~~strikes~~ a command.
#[wrong_channel = "Strike"]
// Serenity will automatically analyse and generate a hint/tip explaining the possible
// cases of ~~strikethrough-commands~~, but only if
// `strikethrough_commands_tip_{dm, guild}` aren't specified.
// If you pass in a value, it will be displayed instead.
// The generated values refer to guilds instead of servers so they're overridden here:
#[dm_only_text("Only in DMs")]
#[guild_only_text("Only in servers")]
#[dm_and_guild_text("In DMs and servers")]
#[strikethrough_commands_tip_in_dm(
	"~~`Strikethrough commands`~~ are unavailable because they only work in servers."
)]
#[strikethrough_commands_tip_in_guild("~~`Strikethrough commands`~~ are unavailable because you (or this bot) do not have the required permissions.")]
async fn my_help(
	context: &Context,
	msg: &Message,
	args: Args,
	help_options: &'static HelpOptions,
	groups: &[&'static CommandGroup],
	owners: HashSet<UserId>,
) -> CommandResult {
	let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
	Ok(())
}

pub fn register(p0: StandardFramework) -> StandardFramework {
	roles::register(dice::register(
		p0
			// Set a function to be called prior to each command execution. This
			// provides the context of the command, the message that was received,
			// and the full name of the command that will be called.
			//
			// You can not use this to determine whether a command should be
			// executed. Instead, the `#[check]` macro gives you this functionality.
			//
			// **Note**: Async closures are unstable, you may use them in your
			// application if you are fine using nightly Rust.
			// If not, we need to provide the function identifiers to the
			// hook-functions (before, after, normal, ...).
			.before(before)
			// Similar to `before`, except will be called directly _after_
			// command execution.
			.after(after)
			// Set a function that's called whenever an attempted command-call's
			// command could not be found.
			.unrecognised_command(unknown_command)
			// Set a function that's called whenever a command's execution didn't complete for one
			// reason or another. For example, when a user has exceeded a rate-limit or a command
			// can only be performed by the bot owner.
			.on_dispatch_error(dispatch_error)
			.group(&GENERAL_GROUP)
			.help(&MY_HELP),
	))
}

#[hook]
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
	// skip bot messages which aren't webhooks
	if msg.author.bot && msg.webhook_id.is_none() {
		return false;
	}

	info!(
		"Got command '{}' by user '{}' on shard {}",
		command_name, msg.author.name, ctx.shard_id
	);

	true // if `before` returns false, command processing doesn't happen.
}

#[hook]
async fn after(ctx: &Context, msg: &Message, command_name: &str, command_result: CommandResult) {
	match command_result {
		Ok(()) => info!("Processed command '{}'", command_name),
		Err(why) => {
			error!("Command '{}' returned error {:?}", command_name, why);
			let _ = super::send_warning_message(ctx, msg.channel_id, &format!("{}", why)).await;
		}
	}
}

#[hook]
async fn unknown_command(ctx: &Context, msg: &Message, unknown_command_name: &str) {
	match msg
		.channel_id
		.say(
			&ctx.http,
			format!("Could not find command named '{}'", unknown_command_name),
		)
		.await
	{
		Ok(_) => {}
		Err(err) => {
			error!(
				"Error sending response to unknown command {}: {:?}",
				unknown_command_name, err
			);
		}
	}
}

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
	let error_message = match error {
		DispatchError::CheckFailed(message, reason) => {
			format!("Check {} failed due to {:?}", message, reason)
		}
		DispatchError::Ratelimited(duration) => format!(
			"Rate limited. Try this again in {} seconds.",
			duration.as_secs()
		),
		DispatchError::LackingPermissions(permission) => {
			format!("Missing permission {}", permission)
		}
		error => format!("Unknown error {:?}", error),
	};

	let _ = msg
		.channel_id
		.send_message(&ctx.http, |m| {
			super::warning_message(m, &error_message);
			m
		})
		.await;
}
