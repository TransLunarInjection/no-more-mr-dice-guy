use super::DiceInt;
use anyhow::{anyhow, ensure, Result};
use regex::Regex;

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Explode {
	Standard,
	Compounding,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Options {
	pub number_of_dice: DiceInt,  // Xd
	pub dice_sides: Vec<DiceInt>, // dX -> [1, 2, ..X]
	pub explode: Option<Explode>,
	pub min: Option<DiceInt>,
	pub max: Option<DiceInt>,
}

pub fn parse(mut str: &str) -> Result<Options> {
	let mut dice_sides: Option<Vec<DiceInt>> = None;
	let mut explode: Option<Explode> = None;
	let mut min = None;
	let mut max = None;

	str = str.trim();
	ensure!(!str.is_empty(), "Can't roll an empty string.");

	let parts = split_keeping_delimiters(&ROLL_OPTION_DELIMITER_REGEX, str);
	let number_of_dice = parts[0]
		.parse::<DiceInt>()
		.map_err(|e| anyhow!("Can't parse number of dice {}, {}", parts[0], e))?;

	let mut idx = 0;
	loop {
		idx += 1;
		if idx >= parts.len() {
			break;
		}
		let option = RollOption::parse(parts[idx])
			.ok_or_else(|| anyhow!("Unknown roll option {}", parts[idx]))?;
		match &option {
			RollOption::Explode(val) => {
				explode = Some(val.clone());
			}
			RollOption::Valued(valued) => {
				idx += 1;

				let value = parts
					.get(idx)
					.ok_or_else(|| anyhow!("Missing value for {:?}", option))?;

				if let (Valued::DiceSides, &"F") = (valued, value) {
					dice_sides = Some(vec![-3, 0, 3]);
					continue;
				}

				let value = value.parse::<DiceInt>()?;
				match valued {
					Valued::DiceSides => {
						ensure!(
							value < super::MAX_DICE_SIDES,
							"Must have < {} dice sides to roll. Tried: {}",
							super::MAX_DICE_SIDES,
							number_of_dice
						);
						dice_sides = Some((1..=value).collect())
					}
					Valued::LessThan => max = Some(value),
					Valued::GreaterThan => min = Some(value),
				}
			}
		}
	}

	let dice_sides = dice_sides.ok_or_else(|| anyhow!("Must set dice sides (eg d20)"))?;

	ensure!(
		!dice_sides.is_empty(),
		"Must have >= 0 sides on dice to roll. Tried: {:?}",
		dice_sides
	);
	ensure!(
		number_of_dice > 0,
		"Must have >= 0 dice to roll. Tried: {}",
		number_of_dice
	);
	ensure!(
		number_of_dice < super::MAX_ROLLED_DICE,
		"Must have < {} dice to roll. Tried: {}",
		super::MAX_ROLLED_DICE,
		number_of_dice
	);
	ensure!(
		dice_sides.len() < (super::MAX_DICE_SIDES as usize),
		"Must have < {} dice sides to roll. Tried: {}",
		super::MAX_DICE_SIDES,
		number_of_dice
	);

	Ok(Options {
		number_of_dice,
		dice_sides,
		explode,
		min,
		max,
	})
}

lazy_static! {
	static ref ROLL_OPTION_DELIMITER_REGEX: Regex =
		Regex::new(r"(\d+|!{1, 2}|F)").expect("Hardcoded regex");
}

fn split_keeping_delimiters<'a>(r: &Regex, text: &'a str) -> Vec<&'a str> {
	let mut result = Vec::new();
	let mut last = 0;
	for (index, matched) in text.match_indices(r) {
		if last != index {
			result.push(&text[last..index]);
		}
		result.push(matched);
		last = index + matched.len();
	}
	if last < text.len() {
		result.push(&text[last..]);
	}
	result
}

#[derive(Debug)]
enum RollOption {
	Valued(Valued),
	Explode(Explode),
}

#[derive(Debug)]
enum Valued {
	DiceSides,
	LessThan,
	GreaterThan,
}

impl RollOption {
	fn parse(str: &str) -> Option<Self> {
		match str {
			"d" => Some(Self::Valued(Valued::DiceSides)),
			"<" => Some(Self::Valued(Valued::LessThan)),
			">" => Some(Self::Valued(Valued::GreaterThan)),
			"!" => Some(Self::Explode(Explode::Standard)),
			"!!" => Some(Self::Explode(Explode::Compounding)),
			_ => None,
		}
	}
}
