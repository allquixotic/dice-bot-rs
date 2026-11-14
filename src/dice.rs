//! Dice expression evaluation and reply formatting. Pure: randomness comes from
//! the `roll` callback so tests are deterministic.

use std::sync::LazyLock;

use regex::Regex;

/// Discord rejects messages longer than this many characters.
pub const DISCORD_MESSAGE_LIMIT: usize = 2000;
const MAX_EXPRESSIONS: usize = 50;
const MAX_TERMS: usize = 70;
const MAX_DICE: i64 = 5_000;
const MAX_SIDES: u32 = 4_000_000;
const MAX_IMPLICIT_ROLLS: u32 = 50;
const NOT_UNDERSTOOD: &str = "Unable to process the supplied dice expression because I didn't understand the dice syntax you supplied.";
const MUST_ROLL_TENS: &str =
  "ERROR: You must roll a d10 dice in this channel, either using the ?ten command or ?roll 1d10.";

/// Whitespace before a sign joins terms: "1d20 +2" means "1d20+2".
static SPACE_BEFORE_SIGN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+([+-])").unwrap());
/// Bare counts/modifiers such as "", "3", "+2" or "3-1" roll the default die.
static IMPLICIT_ROLL: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^\s*(?:(\d*)([+-]?\d+)??)?$").unwrap());

#[derive(Debug, Clone, Copy)]
pub struct RollOptions {
  /// Sides used for implicit rolls (20 normally, 10 for `ten`).
  pub default_sides: u32,
  /// Channel only allows d10s.
  pub d10_only: bool,
  /// Wrap the result in Discord spoiler bars.
  pub spoiler: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Evaluated {
  total: i64,
  /// Individual dice and constants, shown when there is more than one.
  parts: Vec<i64>,
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
  Rolled {
    requested: String,
    results: Vec<Evaluated>,
  },
  NotUnderstood,
  MustRollTens,
}

/// Expands implicit rolls: "" -> "1d20", "+2" -> "1d20+2", "3" -> "1d20 1d20 1d20".
fn expand_implicit(args: &str, default_sides: u32) -> String {
  let Some(caps) = IMPLICIT_ROLL.captures(args) else {
    return args.to_string();
  };
  let modifier = caps.get(2).map_or("", |m| m.as_str());
  let count = caps
    .get(1)
    .and_then(|m| m.as_str().parse::<u32>().ok())
    .unwrap_or(1)
    .max(1);
  if count > 1 {
    let one = format!("1d{default_sides}{modifier}");
    vec![one; count.min(MAX_IMPLICIT_ROLLS) as usize].join(" ")
  } else {
    format!("1d{default_sides}{modifier}")
  }
}

fn evaluate(args: &str, options: RollOptions, roll: &mut impl FnMut(u32) -> u32) -> Outcome {
  let requested = expand_implicit(args, options.default_sides);
  let normalized = SPACE_BEFORE_SIGN
    .replace_all(&requested.to_lowercase(), "$1")
    .into_owned();
  let mut results = Vec::new();

  'expression: for expression in normalized.split_whitespace().take(MAX_EXPRESSIONS) {
    let mut total = 0i64;
    let mut parts = Vec::new();
    let mut has_terms = false;
    for term in expression.replace('-', "+-").split('+').take(MAX_TERMS) {
      if term.is_empty() {
        continue;
      }
      let mut pieces = term.split('d');
      let count_text = pieces.next().unwrap_or_default();
      let count = if count_text.is_empty() {
        1
      } else {
        match count_text.parse::<i64>() {
          Ok(n) => n.clamp(-MAX_DICE, MAX_DICE),
          Err(_) => continue 'expression,
        }
      };
      let sides = match pieces.next() {
        Some(text) => match text.parse::<u32>() {
          Ok(n) => n.min(MAX_SIDES),
          Err(_) => continue 'expression,
        },
        None => 1,
      };
      if pieces.next().is_some() {
        continue 'expression;
      }
      match sides {
        0 => {}
        1 => {
          parts.push(count);
          total += count;
          has_terms = true;
        }
        _ => {
          if options.d10_only && sides != 10 {
            return Outcome::MustRollTens;
          }
          let sign = count.signum();
          for _ in 0..count.abs() {
            let value = i64::from(roll(sides));
            parts.push(value);
            total += sign * value;
          }
          has_terms |= count != 0;
        }
      }
    }
    if has_terms {
      results.push(Evaluated { total, parts });
    }
  }

  if results.is_empty() {
    Outcome::NotUnderstood
  } else {
    Outcome::Rolled { requested, results }
  }
}

fn render_rolled(
  mention: &str,
  requested: &str,
  results: &[Evaluated],
  spoiler: &str,
  show_parts: bool,
) -> String {
  let mut text = String::new();
  for (index, result) in results.iter().enumerate() {
    if index == 0 {
      text.push_str(&format!(
        "{mention} requested {requested} and {spoiler}rolled **{}**",
        result.total
      ));
    } else {
      text.push_str(&format!(", **{}**", result.total));
    }
    if show_parts && result.parts.len() > 1 {
      let parts: Vec<String> = result.parts.iter().map(i64::to_string).collect();
      text.push_str(&format!(" ({})", parts.join(", ")));
    }
  }
  text.push_str(spoiler);
  text.push('.');
  text
}

/// Rolls `args` and formats the reply for `mention` (e.g. "<@123>"), keeping it
/// within Discord's message limit: first drop the individual dice, then shorten
/// the echoed expression. Totals always survive.
pub fn roll_reply(
  mention: &str,
  args: &str,
  options: RollOptions,
  roll: &mut impl FnMut(u32) -> u32,
) -> String {
  match evaluate(args, options, roll) {
    Outcome::NotUnderstood => format!("{mention} {NOT_UNDERSTOOD}"),
    Outcome::MustRollTens => format!("{mention} {MUST_ROLL_TENS}"),
    Outcome::Rolled { requested, results } => {
      let spoiler = if options.spoiler { "||" } else { "" };
      let full = render_rolled(mention, &requested, &results, spoiler, true);
      if full.chars().count() <= DISCORD_MESSAGE_LIMIT {
        return full;
      }
      let totals = render_rolled(mention, &requested, &results, spoiler, false);
      let length = totals.chars().count();
      if length <= DISCORD_MESSAGE_LIMIT {
        return totals;
      }
      let room = requested
        .chars()
        .count()
        .saturating_sub(length - DISCORD_MESSAGE_LIMIT + 1);
      let shortened: String = requested
        .chars()
        .take(room)
        .chain(std::iter::once('…'))
        .collect();
      render_rolled(mention, &shortened, &results, spoiler, false)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const ME: &str = "<@42>";
  const D20: RollOptions = RollOptions {
    default_sides: 20,
    d10_only: false,
    spoiler: false,
  };

  /// Rolls 1, 2, 3, ... (wrapping at the die size) so outputs are predictable.
  fn counting() -> impl FnMut(u32) -> u32 {
    let mut next = 0;
    move |sides| {
      next += 1;
      (next - 1) % sides + 1
    }
  }

  fn max_roll(sides: u32) -> u32 {
    sides
  }

  #[test]
  fn single_die_and_modifier() {
    assert_eq!(
      roll_reply(ME, "1d20", D20, &mut counting()),
      "<@42> requested 1d20 and rolled **1**."
    );
    assert_eq!(
      roll_reply(ME, "1d20+5", D20, &mut counting()),
      "<@42> requested 1d20+5 and rolled **6** (1, 5)."
    );
    assert_eq!(
      roll_reply(ME, "2d6-1", D20, &mut counting()),
      "<@42> requested 2d6-1 and rolled **2** (1, 2, -1)."
    );
  }

  #[test]
  fn several_expressions_and_spaced_signs() {
    assert_eq!(
      roll_reply(ME, "1D20 +2 1d20  +3", D20, &mut counting()),
      "<@42> requested 1D20 +2 1d20  +3 and rolled **3** (1, 2), **5** (2, 3)."
    );
  }

  #[test]
  fn implicit_rolls_use_the_default_die() {
    assert_eq!(
      roll_reply(ME, "", D20, &mut max_roll),
      "<@42> requested 1d20 and rolled **20**."
    );
    assert_eq!(
      roll_reply(ME, "+2", D20, &mut max_roll),
      "<@42> requested 1d20+2 and rolled **22** (20, 2)."
    );
    assert_eq!(
      roll_reply(ME, "3", D20, &mut max_roll),
      "<@42> requested 1d20 1d20 1d20 and rolled **20**, **20**, **20**."
    );
    let ten = RollOptions {
      default_sides: 10,
      ..D20
    };
    assert_eq!(
      roll_reply(ME, "-1", ten, &mut max_roll),
      "<@42> requested 1d10-1 and rolled **9** (10, -1)."
    );
  }

  #[test]
  fn implicit_roll_count_is_capped() {
    let reply = roll_reply(ME, "500", D20, &mut max_roll);
    assert_eq!(reply.matches("**20**").count(), 50);
  }

  #[test]
  fn negative_dice_subtract() {
    assert_eq!(
      roll_reply(ME, "1d20-1d4", D20, &mut max_roll),
      "<@42> requested 1d20-1d4 and rolled **16** (20, 4)."
    );
  }

  #[test]
  fn spoiler_wraps_the_result() {
    let spoiler = RollOptions {
      spoiler: true,
      ..D20
    };
    assert_eq!(
      roll_reply(ME, "1d6", spoiler, &mut max_roll),
      "<@42> requested 1d6 and ||rolled **6**||."
    );
  }

  #[test]
  fn nonsense_is_reported() {
    let expected = format!("<@42> {NOT_UNDERSTOOD}");
    assert_eq!(roll_reply(ME, "fireball", D20, &mut max_roll), expected);
    assert_eq!(roll_reply(ME, "1d6d6", D20, &mut max_roll), expected);
    assert_eq!(roll_reply(ME, "0d6", D20, &mut max_roll), expected);
  }

  #[test]
  fn empty_leading_expression_keeps_the_mention() {
    // Previously "0d6 1d20" produced ", **20**." with no mention.
    assert_eq!(
      roll_reply(ME, "0d6 1d20", D20, &mut max_roll),
      "<@42> requested 0d6 1d20 and rolled **20**."
    );
  }

  #[test]
  fn d10_only_channels_reject_other_dice() {
    let d10 = RollOptions {
      d10_only: true,
      ..D20
    };
    assert_eq!(
      roll_reply(ME, "1d20", d10, &mut max_roll),
      format!("<@42> {MUST_ROLL_TENS}")
    );
    assert_eq!(
      roll_reply(ME, "2d10+3", d10, &mut max_roll),
      "<@42> requested 2d10+3 and rolled **23** (10, 10, 3)."
    );
  }

  #[test]
  fn huge_totals_do_not_overflow() {
    let reply = roll_reply(ME, "5000d4000000", D20, &mut max_roll);
    assert!(reply.contains("rolled **20000000000**"), "{reply}");
  }

  #[test]
  fn long_replies_drop_dice_then_shorten_the_echo() {
    assert_eq!(
      roll_reply(ME, "700d6", D20, &mut max_roll),
      "<@42> requested 700d6 and rolled **4200**."
    );

    let long = format!("{}1d6", "1d6+".repeat(495));
    let spoiler = RollOptions {
      spoiler: true,
      ..D20
    };
    let reply = roll_reply(ME, &long, spoiler, &mut max_roll);
    assert_eq!(reply.chars().count(), DISCORD_MESSAGE_LIMIT);
    assert!(reply.ends_with("… and ||rolled **420**||."), "{reply}");
  }

  #[test]
  fn constants_are_clamped() {
    assert_eq!(
      roll_reply(ME, "1d1+99999", D20, &mut max_roll),
      "<@42> requested 1d1+99999 and rolled **5001** (1, 5000)."
    );
  }
}
