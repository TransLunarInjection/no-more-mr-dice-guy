use rand::Rng;
use regex::{Captures, Regex, Replacer};
use std::borrow::Cow;

use anyhow::{anyhow, ensure, Result};
use std::str::Chars;

lazy_static! {
	static ref ROLL_REGEX: Regex = Regex::new(r"(^|[+\- (])(\d+d[^+\- )]+)($|[$+\- )])").unwrap();
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
		regex_replace_all_overlapping(&ROLL_REGEX, Cow::from(msg), |caps: &Captures| {
			let rep = format!("{}{}{}", &caps[1], rolls[idx].val(), &caps[3]);
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
	number_of_dice: u32, // Xd
	dice_size: u32,      // dX
	explode: Option<Explode>,
	min: Option<u32>,
	max: Option<u32>,
	rolls: Vec<u32>,
}

#[derive(Eq, PartialEq, Debug)]
enum Explode {
	Standard,
	Compounding,
}

impl DiceRoll {
	fn from_str(str: &str, rng: &mut impl Rng) -> Result<DiceRoll> {
		let mut iter = str.chars();
		let mut ty = '_';
		let mut number_of_dice = 1;
		let mut dice_size = 1;
		let mut explode: Option<Explode> = None;
		let mut min = None;
		let mut max = None;

		loop {
			let (new_iter, val, next_type) = parse_int_until(iter);
			let val = val.unwrap_or(1);
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

		let mut rolls = vec![];
		let mut dice_to_roll = number_of_dice;
		while dice_to_roll > 0 {
			let mut maxed = 0;
			for _ in 0..number_of_dice {
				let mut current_roll = rng.gen_range(1, dice_size + 1);
				let mut total = current_roll;
				if current_roll == dice_size {
					maxed += 1;
					if explode == Some(Explode::Compounding) {
						while current_roll == dice_size {
							current_roll = rng.gen_range(1, dice_size + 1);
							total += current_roll;
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

		Ok(DiceRoll {
			number_of_dice,
			dice_size,
			explode,
			min,
			max,
			rolls,
		})
	}

	fn check_dice(&self, dice: u32) -> bool {
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
					(if !self.check_dice(*it) {
						format!("~~{}~~", it)
					} else {
						it.to_string()
					}) + ", "
				})
				.collect::<String>()
				.trim_end_matches(", ")
		)
	}

	fn val(&self) -> i32 {
		self.rolls
			.iter()
			.filter(|it| self.check_dice(**it))
			.sum::<u32>() as i32
	}
}

fn parse_int_until(mut chars: Chars) -> (Chars, Option<u32>, Option<char>) {
	let mut int_chars = String::new();
	let end = loop {
		match chars.next() {
			None => break None,
			Some(chr) if chr.is_numeric() => int_chars.push(chr),
			Some(chr) => break Some(chr),
		}
	};
	(chars, int_chars.parse().ok(), end)
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn check_regex() {
		assert!(ROLL_REGEX.is_match("(1d20)"));
		assert!(ROLL_REGEX.is_match("1d20"));
		assert!(ROLL_REGEX.is_match("2 + 1d20"));
		assert!(ROLL_REGEX.is_match("4 + (5d11 / 2)"));
	}

	#[test]
	fn dice_roll_from_str() -> Result<()> {
		assert_eq!(
			DiceRoll::from_str("1d1", &mut rand::thread_rng())?,
			DiceRoll {
				number_of_dice: 1,
				dice_size: 1,
				explode: None,
				min: None,
				max: None,
				rolls: vec![1]
			}
		);
		Ok(())
	}

	fn test_rng() -> impl Rng {
		use rand::SeedableRng;
		rand::rngs::StdRng::from_seed([0; 32])
	}

	#[test]
	fn roll_expression_simple() -> Result<()> {
		assert_eq!(
			roll_expressions("(1d1+1d1)", &mut test_rng())?,
			("([1]+[1])".to_string(), "(1+1)".to_string())
		);
		assert_eq!(
			roll_expressions("(1d1 + 1d1)", &mut test_rng())?,
			("([1] + [1])".to_string(), "(1 + 1)".to_string())
		);
		assert_eq!(
			roll_expressions("(5d11<5)", &mut test_rng())?,
			(
				"([~~8~~, ~~7~~, 2, ~~9~~, ~~6~~])".to_string(),
				"(2)".to_string()
			)
		);
		Ok(())
	}

	#[test]
	fn roll_negative() {
		assert!(roll_expression("-1d-1").is_err());
	}
}
