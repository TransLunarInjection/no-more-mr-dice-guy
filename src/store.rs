use crate::prelude::*;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serenity::futures::io::ErrorKind;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tokio::task::JoinHandle;

pub async fn load_data<T: 'static + Send + Default + for<'de> serde::de::Deserialize<'de>>(
	id: &str,
) -> Result<T> {
	use std::fs::File;

	let loc = location::<T>(id);

	let result = tokio::task::spawn_blocking(move || {
		let file = match File::open(&loc) {
			Ok(ok) => Ok(ok),
			Err(e) => match e.kind() {
				ErrorKind::NotFound => {
					return Ok(Default::default());
				}
				_ => Err(e).with_context(|| format!("Failed to open {}", loc.display())),
			},
		}?;
		Ok(serde_json::from_reader(BufReader::new(file))?)
	})
	.await;

	result?
}

pub fn save_data<T: 'static + Send + Serialize + Sync>(id: &str, data: T) -> JoinHandle<()> {
	let loc = location::<T>(id);

	tokio::task::spawn_blocking(move || match save_data_replace(&loc, &data) {
		Ok(_) => {}
		Err(err) => {
			error!("Failed to save {} due to {:?}", loc.display(), err);
		}
	})
}

fn save_data_replace<T: Serialize>(loc: impl AsRef<Path>, data: &T) -> Result<()> {
	use std::fs::File;

	let loc = loc.as_ref();
	std::fs::create_dir_all(
		loc.parent()
			.ok_or_else(|| anyhow!("loc should have parent {:?}", &loc))?,
	)?;

	let temp_loc = loc.with_extension(".tmp");

	// this block is meaningful - file closes when it's dropped
	{
		let mut file = File::create(&temp_loc)?;
		serde_json::to_writer_pretty(&mut file, data)?;
		file.sync_all()?;
	}

	std::fs::rename(temp_loc, loc)?;

	Ok(())
}

fn location<T>(id: &str) -> PathBuf {
	let mut location = PathBuf::new();
	location.push("store");
	let type_name = std::any::type_name::<T>();
	location.push(
		type_name
			.chars()
			.map(|x| if x.is_alphanumeric() { x } else { '_' })
			.collect::<String>(),
	);
	location.push(id);
	location
}
