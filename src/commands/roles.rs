use super::prelude::*;
use crate::RoleData;
use anyhow::{anyhow, Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use serenity::builder::{CreateEmbed, EditMessage};
use serenity::framework::standard::{Args, CommandResult};
use serenity::framework::StandardFramework;
use std::collections::HashMap;
use std::sync::Arc;

pub fn register(framework: StandardFramework) -> StandardFramework {
	framework.group(&ROLES_GROUP)
}

pub struct Persistent {
	guild_data: Arc<RwLock<HashMap<GuildId, RolesConfig>>>,
}

impl Default for Persistent {
	fn default() -> Self {
		Self {
			guild_data: Arc::new(RwLock::default()),
		}
	}
}

impl Persistent {
	async fn from_context(ctx: &Context) -> Arc<Self> {
		ctx.data
			.read()
			.await
			.get::<RoleData>()
			.expect("RoleData is initialised at start")
			.clone()
	}

	async fn get_guild_data(&self, id: GuildId) -> Result<RolesConfig> {
		{
			if let Some(data) = self.guild_data.read().await.get(&id) {
				return Ok(data.clone());
			}
		}

		let loaded = crate::store::load_data::<RolesConfig>(&id.0.to_string()).await?;

		Ok({
			let mut dat = self.guild_data.write().await;
			dat.entry(id).or_insert_with(|| loaded).clone()
		})
	}

	async fn set_guild_data(&self, id: GuildId, data: RolesConfig) {
		let _ = crate::store::save_data::<RolesConfig>(&id.0.to_string(), data.clone());

		{
			let mut dat = self.guild_data.write().await;
			dat.insert(id, data);
		}
	}
}

pub async fn handle_reaction(ctx: &Context, reaction: &Reaction, added: bool) -> Result<()> {
	let id = match reaction.emoji {
		ReactionType::Custom { id, .. } => id,
		_ => return Ok(()),
	};
	let user_id = match reaction.user_id {
		None => return Ok(()),
		Some(user_id) => user_id,
	};

	if user_id == ctx.cache.current_user_id().await {
		return Ok(());
	}

	let guild_id = if let Some(guild_id) = reaction.guild_id {
		guild_id
	} else {
		return Ok(());
	};

	let persistent = Persistent::from_context(ctx).await;
	let cfg = persistent.get_guild_data(guild_id).await?;

	let current_role_message = if let Some(current_role_message) = cfg.current_role_message {
		current_role_message
	} else {
		return Ok(());
	};

	if reaction.message_id == current_role_message.1 {
		if let Some(role) = cfg.roles.iter().find(|x| x.1.id == id) {
			let mut member: Member = match guild_id.member(&ctx, user_id).await {
				Ok(member) => member,
				Err(_) => return Ok(()),
			};

			let add = if added {
				member.add_role(&ctx, role.0).await
			} else {
				member.remove_role(&ctx, role.0).await
			};

			if let Err(e) = add {
				error!(
					"Error adding role {} to {} due to {:?}",
					role.0.to_string(),
					member.user.name,
					e
				)
			}
		}
	}

	Ok(())
}

#[group]
#[prefix(roles)]
#[commands(add_role_toggle, create_toggle_message, remove_role_toggle)]
struct Roles;

#[command]
#[checks(ManageRolesHigh)]
async fn add_role_toggle(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	args.trimmed();
	let emoji: EmojiIdentifier =
		serenity::utils::parse_emoji(args.current().ok_or_else(|| anyhow!("Missing emoji"))?)
			.context("Couldn't parse first argument as emoji")?;
	args.advance();
	let role: &str = args
		.remains()
		.ok_or_else(|| anyhow!("Missing role name"))?
		.trim();

	let guild: Guild = msg
		.guild(&ctx)
		.await
		.ok_or_else(|| anyhow!("Couldn't retrieve guild"))?;
	let role = guild
		.role_by_name(role)
		.ok_or_else(|| anyhow!("Couldn't find role {}", role))?;

	{
		let persistent = Persistent::from_context(ctx).await;
		let mut cfg = persistent.get_guild_data(guild.id).await?;

		cfg.roles.retain(|f| f.0 != role.id && f.1 != emoji);
		cfg.roles.push(RoleEmoji(role.id, emoji));

		persistent.set_guild_data(guild.id, cfg).await;
	}

	update_or_create_toggle_message(ctx, msg, false).await
}

#[command]
#[checks(ManageRolesHigh)]
async fn remove_role_toggle(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	args.trimmed();
	let emoji: EmojiIdentifier =
		serenity::utils::parse_emoji(args.current().ok_or_else(|| anyhow!("Missing emoji"))?)
			.context("Couldn't parse first argument as emoji")?;

	let guild: Guild = msg
		.guild(&ctx)
		.await
		.ok_or_else(|| anyhow!("Couldn't retrieve guild"))?;

	{
		let persistent = Persistent::from_context(ctx).await;
		let mut cfg = persistent.get_guild_data(guild.id).await?;

		cfg.roles.retain(|f| f.1 != emoji);

		persistent.set_guild_data(guild.id, cfg).await;
	}

	update_or_create_toggle_message(ctx, msg, false).await
}

#[command]
#[checks(ManageRolesHigh)]
async fn create_toggle_message(ctx: &Context, msg: &Message) -> CommandResult {
	update_or_create_toggle_message(ctx, msg, true).await
}

async fn update_or_create_toggle_message(
	ctx: &Context,
	command_msg: &Message,
	allow_creation: bool,
) -> CommandResult {
	let guild = command_msg
		.guild_id
		.ok_or_else(|| anyhow!("Must be in a guild"))?;
	let mut cfg = {
		Persistent::from_context(ctx)
			.await
			.get_guild_data(guild)
			.await?
	};

	if allow_creation && cfg.roles.is_empty() {
		return Err(anyhow!("Guild must have role toggles configured").into());
	}

	command_msg.channel_id.broadcast_typing(&ctx.http).await?;

	let existing_message = match cfg.current_role_message {
		None => None,
		Some((channel_id, message_id)) => match channel_id.message(&ctx, message_id).await.ok() {
			None => None,
			Some(message) => {
				if message.author.id == ctx.cache.current_user_id().await {
					Some(message)
				} else {
					None
				}
			}
		},
	};

	let created_message = existing_message.is_none();
	if created_message && !allow_creation {
		return Ok(());
	}

	let mut existing_message = match existing_message {
		Some(message) => message,
		None => {
			command_msg
				.channel_id
				.send_message(&ctx, |m| {
					m.embed(|e| {
						e.color(crate::COLOR);
						e.title("Initialising");

						e
					});

					m
				})
				.await?
		}
	};

	existing_message
		.edit(&ctx, |m: &mut EditMessage| {
			m.embed(|e: &mut CreateEmbed| {
				e.color(crate::COLOR);
				e.title("Choose your roles");
				e.description(role_choices(&cfg));
				e
			});
			m
		})
		.await?;

	// failure is okay here - don't mind if can't remove old reacts
	let _ = existing_message.delete_reactions(&ctx).await;

	setup_reactions(ctx, &existing_message, &cfg).await?;

	{
		let id = Some((existing_message.channel_id, existing_message.id));
		if cfg.current_role_message != id {
			cfg.current_role_message = id;
		}
		Persistent::from_context(ctx)
			.await
			.set_guild_data(guild, cfg)
			.await;
	}

	if !created_message {
		existing_message.guild_id = Some(guild);
		command_msg
			.channel_id
			.send_message(&ctx, |m| {
				m.embed(|e| {
					e.color(crate::COLOR);
					e.description(format!(
						"Updated [role chooser]({})",
						existing_message.link()
					));

					e
				});

				m
			})
			.await?;
	}

	Ok(())
}

async fn setup_reactions(ctx: &Context, msg: &Message, cfg: &RolesConfig) -> CommandResult {
	for RoleEmoji(_, emoji) in &cfg.roles {
		msg.react(&ctx, emoji.clone()).await?;
	}

	Ok(())
}

fn role_choices(cfg: &RolesConfig) -> String {
	let mut str = String::new();

	for RoleEmoji(role_id, emoji_id) in &cfg.roles {
		str += &format!("{} <@&{}>\n", emoji_id_to_str(emoji_id), role_id);
	}

	str
}

fn emoji_id_to_str(e: &EmojiIdentifier) -> String {
	let mut result = String::new();
	result.push('<');

	if e.animated {
		result.push('a');
	}

	result.push(':');
	result.push_str(&e.name);
	result.push(':');
	result.push_str(&format!("{}", e.id.0));
	result.push('>');

	result
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct RolesConfig {
	current_role_message: Option<(ChannelId, MessageId)>,
	roles: Vec<RoleEmoji>,
}

#[derive(Clone, Serialize, Deserialize)]
struct RoleEmoji(
	#[serde(with = "RoleIdDef")] RoleId,
	#[serde(with = "EmojiIdentifierDef")] EmojiIdentifier,
);

#[derive(Serialize, Deserialize)]
#[serde(remote = "RoleId")]
struct RoleIdDef(pub u64);

#[derive(Serialize, Deserialize)]
#[serde(remote = "EmojiIdentifier")]
struct EmojiIdentifierDef {
	#[serde(default)]
	pub animated: bool,
	pub id: EmojiId,
	pub name: String,
}
