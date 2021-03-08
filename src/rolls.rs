use anyhow::{anyhow, Result};
use rand::Rng;
use regex::{Captures, Regex, Replacer};
use std::borrow::Cow;

pub type DiceInt = i32;

mod options;
#[cfg(test)]
mod test;

use options::Options;

const MAX_ROLLED_DICE: DiceInt = 500;
const MAX_DICE_SIDES: DiceInt = 10_000;

lazy_static! {
	static ref ROLL_REGEX: Regex =
		Regex::new(r"(^|[+\- (])(\d+d[^+\-\*/ )]+)($|[$+\-\*/ )])").expect("Hardcoded regex");
}

pub fn roll_expression_value(msg: &str) -> Result<DiceInt> {
	use num_traits::cast::ToPrimitive;
	let (_, _, vals) = roll_expressions(msg, &mut rand::thread_rng())?;
	let evaled = meval::eval_str(&vals).map_err(|_e| anyhow!("Couldn't evaluate {}", vals))?;
	Ok(evaled
		.to_i32()
		.ok_or_else(|| anyhow!("Result out of range"))?)
}

pub fn roll_expression(msg: &str) -> Result<String> {
	let (rolls, dice, vals) = roll_expressions(msg, &mut rand::thread_rng())?;

	let single_simple_roll = rolls.len() == 1
		&& rolls[0].options.explode.is_none()
		&& rolls[0].options.number_of_dice == 1
		&& vals.parse::<DiceInt>().is_ok();

	Ok(if single_simple_roll {
		format!("**{}**", vals)
	} else {
		let evaled = meval::eval_str(&vals).map_err(|_e| anyhow!("Couldn't evaluate {}", vals))?;
		format!("{} => **{}**", dice, evaled)
	})
}

fn roll_expressions(msg: &str, rng: &mut impl Rng) -> Result<(Vec<DiceRoll>, String, String)> {
	let mut rolls = vec![];

	let result_rolled = {
		let mut idx = 0;
		let mut err = Ok(());
		let result =
			regex_replace_all_overlapping(&ROLL_REGEX, Cow::from(msg), |caps: &Captures| {
				let roll = match DiceRoll::from_str(&caps[2], rng) {
					Ok(roll) => roll,
					Err(e) => {
						err = Err(e);
						return "".to_string();
					}
				};
				let rep = format!("{}{}{}", &caps[1], roll.dice(), &caps[3]);
				rolls.push(roll);
				idx += 1;
				rep
			});
		err?;
		result
	};

	let result_valued = {
		let mut idx = 0;
		let sums: Vec<DiceInt> = rolls
			.iter()
			.map(DiceRoll::val)
			.collect::<Result<Vec<DiceInt>>>()?;
		regex_replace_all_overlapping(&ROLL_REGEX, Cow::from(msg), |caps: &Captures| {
			let rep = format!("{}{}{}", &caps[1], sums[idx], &caps[3]);
			idx += 1;
			rep
		})
	};

	Ok((rolls, result_rolled, result_valued))
}

fn regex_replace_all_overlapping(
	regex: &Regex,
	mut msg: Cow<str>,
	mut replacer: impl Replacer,
) -> String {
	loop {
		match regex.replace(&msg, replacer.by_ref()) {
			Cow::Borrowed(_) => break,
			Cow::Owned(owned) => msg = Cow::from(owned),
		}
	}
	msg.to_string()
}

// A single NdN roll eg 3d20 -> [1, 5, 20]
#[derive(Eq, PartialEq, Debug)]
struct DiceRoll {
	options: Options,
	rolls: Vec<DiceInt>,
}

impl DiceRoll {
	fn from_str(str: &str, rng: &mut impl Rng) -> Result<Self> {
		let options = options::parse(str)?;

		let dice_size_bound = options.dice_sides.len();

		let max_possible_roll = *options
			.dice_sides
			.iter()
			.max()
			.ok_or_else(|| anyhow!("Must have at least one dice side"))?;

		let mut rolls = vec![];
		let mut dice_to_roll = options.number_of_dice;
		while dice_to_roll > 0 {
			// dice which hit max value which need exploded
			let mut maxed: DiceInt = 0;
			for _ in 0..dice_to_roll {
				let mut current_roll = options.dice_sides[rng.gen_range(0..dice_size_bound)];
				let mut total = current_roll;
				if current_roll == max_possible_roll && options.explode != None {
					maxed = maxed.checked_add(1).ok_or_else(|| {
						anyhow!("Overflow due to overflow tracking exploded dice count.")
					})?;
					if options.explode == Some(options::Explode::Compounding) {
						while current_roll == max_possible_roll {
							current_roll = options.dice_sides[rng.gen_range(0..dice_size_bound)];
							total = total.checked_add(current_roll).ok_or_else(|| {
								anyhow!("Overflow due to overflow during compounded explode")
							})?;
						}
					}
				}
				rolls.push(total);
			}

			dice_to_roll = 0;
			if options.explode == Some(options::Explode::Standard) {
				dice_to_roll = maxed;
			}
		}

		Ok(Self { options, rolls })
	}

	const fn check_dice(&self, dice: DiceInt) -> bool {
		if let Some(min) = self.options.min {
			if dice <= min {
				return false;
			}
		}
		if let Some(max) = self.options.max {
			if dice >= max {
				return false;
			}
		}

		true
	}

	fn dice(&self) -> String {
		format!(
			"[{}]",
			self.rolls
				.iter()
				.map(|it| {
					(if self.check_dice(*it) {
						it.to_string()
					} else {
						format!("~~{}~~", it)
					}) + ", "
				})
				.collect::<String>()
				.trim_end_matches(", ")
		)
	}

	fn val(&self) -> Result<DiceInt> {
		let mut sum: DiceInt = 0;
		for roll in &self.rolls {
			if self.check_dice(*roll) {
				sum = sum
					.checked_add(*roll)
					.ok_or_else(|| anyhow!("Overflow summing dice values"))?;
			}
		}
		Ok(sum)
	}
}
