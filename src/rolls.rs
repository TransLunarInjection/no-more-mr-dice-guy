use rand::Rng;
use regex::{Captures, Regex, Replacer};
use std::borrow::Cow;

use anyhow::{anyhow, ensure, Result};
use std::str::Chars;

type DiceInt = u32;

#[cfg(test)]
mod test;

const MAX_ROLLED_DICE: DiceInt = 10_000;
const MAX_DICE_SIDES: DiceInt = 10_000;

lazy_static! {
	static ref ROLL_REGEX: Regex =
		Regex::new(r"(^|[+\- (])(\d+d[^+\- )]+)($|[$+\- )])").expect("Hardcoded regex");
}

pub fn roll_expression(msg: &str) -> Result<String> {
	let (dice, vals) = roll_expressions(msg, &mut rand::thread_rng())?;
	let evaled = meval::eval_str(&vals).map_err(|_e| anyhow!("Couldn't evaluate {}", vals))?;
	Ok(format!("{} => **{}**", dice, evaled))
}

pub fn roll_expressions(msg: &str, rng: &mut impl Rng) -> Result<(String, String)> {
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

	Ok((result_rolled, result_valued))
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
	number_of_dice: DiceInt, // Xd
	dice_size: DiceInt,      // dX
	explode: Option<Explode>,
	min: Option<DiceInt>,
	max: Option<DiceInt>,
	rolls: Vec<DiceInt>,
}

#[derive(Eq, PartialEq, Debug)]
enum Explode {
	Standard,
	Compounding,
}

impl DiceRoll {
	fn from_str(str: &str, rng: &mut impl Rng) -> Result<Self> {
		let mut iter = str.chars();
		let mut ty = '_';
		let mut number_of_dice = 1;
		let mut dice_size = 1;
		let mut explode: Option<Explode> = None;
		let mut min = None;
		let mut max = None;

		loop {
			let (new_iter, val, next_type) = parse_int_until(iter);
			let val = val?;
			match ty {
				'_' => {
					number_of_dice = val;
				}
				'd' => dice_size = val,
				'!' => {
					if explode.is_none() {
						explode = Some(Explode::Standard);
					} else {
						explode = Some(Explode::Compounding);
					}
				}
				'<' => {
					max = Some(val);
				}
				'>' => {
					min = Some(val);
				}
				_ => return Err(anyhow!("Unknown roll character '{}'", ty)),
			}
			iter = new_iter;
			ty = match next_type {
				Some(next_type) => next_type,
				None => break,
			}
		}

		ensure!(
			dice_size > 0,
			"Must have >= 0 sides on dice to roll. Tried: {}",
			dice_size
		);
		ensure!(
			number_of_dice > 0,
			"Must have >= 0 dice to roll. Tried: {}",
			number_of_dice
		);
		ensure!(
			number_of_dice < MAX_ROLLED_DICE,
			"Must have < {} dice to roll. Tried: {}",
			MAX_ROLLED_DICE,
			number_of_dice
		);
		ensure!(
			dice_size < MAX_DICE_SIDES,
			"Must have < {} dice to roll. Tried: {}",
			MAX_DICE_SIDES,
			number_of_dice
		);

		let dice_size_bound = dice_size
			.checked_add(1)
			.ok_or_else(|| anyhow!("Overflow rolling with sides {}", dice_size))?;

		let mut rolls = vec![];
		let mut dice_to_roll = number_of_dice;
		while dice_to_roll > 0 {
			// dice which hit max value which need exploded
			let mut maxed: DiceInt = 0;
			for _ in 0..number_of_dice {
				let mut current_roll = rng.gen_range(1, dice_size_bound);
				let mut total = current_roll;
				if current_roll == dice_size && explode != None {
					maxed = maxed.checked_add(1).ok_or_else(|| {
						anyhow!("Overflow due to overflow tracking exploded dice count.")
					})?;
					if explode == Some(Explode::Compounding) {
						while current_roll == dice_size {
							current_roll = rng.gen_range(1, dice_size_bound);
							total = total.checked_add(current_roll).ok_or_else(|| {
								anyhow!("Overflow due to overflow during compounded explode")
							})?;
						}
					}
				}
				rolls.push(total);
			}

			dice_to_roll = 0;
			if explode == Some(Explode::Standard) {
				dice_to_roll = maxed;
			}
		}

		Ok(Self {
			number_of_dice,
			dice_size,
			explode,
			min,
			max,
			rolls,
		})
	}

	const fn check_dice(&self, dice: DiceInt) -> bool {
		if let Some(min) = self.min {
			if dice <= min {
				return false;
			}
		}
		if let Some(max) = self.max {
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

fn parse_int_until(mut chars: Chars) -> (Chars, Result<u32>, Option<char>) {
	let mut int_chars = String::new();
	let end = loop {
		match chars.next() {
			None => break None,
			Some(chr) if chr.is_numeric() => int_chars.push(chr),
			Some(chr) => break Some(chr),
		}
	};
	(
		chars,
		int_chars.parse::<DiceInt>().map_err(|e| anyhow!("{}", e)),
		end,
	)
}
