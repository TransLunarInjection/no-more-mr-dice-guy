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
			options: Options {
				number_of_dice: 1,
				dice_sides: 1,
				explode: None,
				min: None,
				max: None
			},
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
fn roll_expression_exploded() -> Result<()> {
	assert_eq!(
		roll_expressions("11d3!", &mut test_rng())?,
		(
			"[3, 2, 3, 1, 3, 1, 3, 3, 2, 2, 1, 1, 1, 2, 2, 3, 2]".to_string(),
			"35".to_string()
		)
	);
	Ok(())
}

#[test]
fn roll_expression_compounded() -> Result<()> {
	assert_eq!(
		roll_expressions("11d3!!", &mut test_rng())?,
		(
			"[5, 4, 4, 8, 2, 1, 1, 1, 2, 2, 5]".to_string(),
			"35".to_string()
		)
	);
	Ok(())
}

#[test]
fn roll_negative() {
	assert!(roll_expression("-1d-1").is_err());
}

#[test]
fn roll_oversized_sides() {
	assert!(roll_expression("2d4294967295").is_err());
}

#[test]
fn roll_oversized_dice() {
	assert!(roll_expression("4294967295d2").is_err());
}

#[test]
fn roll_oversized_result() {
	assert!(roll_expression("999999999d999999999").is_err());
}

#[test]
fn roll_oversized_unparseable() {
	{
		let roll = roll_expression("999999999999999999d1");
		assert!(roll.is_err(), "{:?}", roll);
	}
	{
		let roll = roll_expression("1d999999999999999999999999");
		assert!(roll.is_err(), "{:?}", roll);
	}
}

#[test]
fn roll_barely_acceptably_sized() {
	{
		let roll = roll_expression(&format!("{}d1", MAX_ROLLED_DICE - 1));
		assert!(roll.is_ok(), "{:?}", roll);
	}
	{
		let roll = roll_expression(&format!("1d{}", MAX_DICE_SIDES - 1));
		assert!(roll.is_ok(), "{:?}", roll);
	}
	{
		let roll = roll_expression(&format!("{}d{}", MAX_ROLLED_DICE - 1, MAX_DICE_SIDES - 1));
		assert!(roll.is_ok(), "{:?}", roll);
	}
}

#[test]
fn handle_overflow_in_sum() {
	assert!(DiceRoll {
		options: Options {
			number_of_dice: 1,
			dice_sides: 1,
			explode: None,
			min: None,
			max: None
		},
		rolls: vec![DiceInt::max_value() - 1, 2]
	}
	.val()
	.is_err())
}
