//! Prefix command parsing and replies: "?roll 2d6", "? ten +1", "<@bot> rollsp", "?roll3".

use std::{collections::HashSet, sync::LazyLock};

use regex::Regex;

use crate::dice::{self, RollOptions};

const YELL: &str = "Wow, okay! Is your caps lock on, or are you mad at me? :( ";
/// "roll3", "rollsp+2", "ten3-1": a command with its count/modifier glued on.
static GLUED: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(roll|ten)(sp)?(\d*[+-]?\d+)$").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
  Roll,
  Ten,
  RollSpoiler,
  TenSpoiler,
  Help,
}

impl Command {
  fn from_name(name: &str) -> Option<Self> {
    Some(match name {
      "roll" | "dice" => Self::Roll,
      "ten" => Self::Ten,
      "rollsp" => Self::RollSpoiler,
      "tensp" => Self::TenSpoiler,
      "help" => Self::Help,
      _ => return None,
    })
  }

  /// Typing the command in capitals earns a remark, as it always has.
  fn shouted(self, word: &str) -> bool {
    let loud = match self {
      Self::Roll => "ROLL",
      Self::Ten => "TEN",
      Self::RollSpoiler => "ROLLSP",
      Self::TenSpoiler => "TENSP",
      Self::Help => return false,
    };
    word.contains(loud)
  }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
  pub command: Command,
  pub args: String,
  pub yelling: bool,
}

fn strip_mention(content: &str, bot_id: u64) -> Option<&str> {
  [format!("<@{bot_id}>"), format!("<@!{bot_id}>")]
    .iter()
    .find_map(|mention| content.strip_prefix(mention.as_str()))
}

/// Parses a message; `None` when it is not addressed to the bot or is not a command.
pub fn parse(content: &str, prefixes: &[String], bot_id: Option<u64>) -> Option<Invocation> {
  let rest = prefixes
    .iter()
    .find_map(|prefix| content.strip_prefix(prefix.as_str()))
    .or_else(|| bot_id.and_then(|id| strip_mention(content, id)))?
    .trim_start();
  let (word, args) = match rest.find(char::is_whitespace) {
    Some(split) => (&rest[..split], rest[split..].trim()),
    None => (rest, ""),
  };
  let lower = word.to_lowercase();
  if let Some(command) = Command::from_name(&lower) {
    return Some(Invocation {
      command,
      args: args.to_string(),
      yelling: command.shouted(word),
    });
  }
  let caps = GLUED.captures(&lower).filter(|_| args.is_empty())?;
  let command = match (&caps[1], caps.get(2).is_some()) {
    ("roll", false) => Command::Roll,
    ("roll", true) => Command::RollSpoiler,
    ("ten", false) => Command::Ten,
    _ => Command::TenSpoiler,
  };
  Some(Invocation {
    command,
    args: caps[3].to_string(),
    yelling: command.shouted(word),
  })
}

#[derive(Debug, Clone)]
pub struct Settings {
  pub prefixes: Vec<String>,
  pub default_sides: u32,
  pub d10_only_channels: HashSet<u64>,
}

pub fn help_text(prefixes: &[String]) -> String {
  let p = prefixes.first().map_or("?", String::as_str);
  let all: Vec<String> = prefixes
    .iter()
    .map(|prefix| format!("`{prefix}`"))
    .collect();
  format!(
    "**Dice commands** (prefix {}, or mention me):\n\
     `{p}roll EXPR…` (or `{p}dice`): roll dice, e.g. `{p}roll 2d6+3 1d20`. Bare numbers use a d20: `{p}roll`, `{p}roll +2`, `{p}roll 3`.\n\
     `{p}ten EXPR…`: the same, but bare numbers use d10s.\n\
     `{p}rollsp` / `{p}tensp`: the same, with the result hidden in a spoiler.\n\
     Shortcuts: `{p}roll3`, `{p}roll+2`, `{p}ten+1`.",
    all.join(" ")
  )
}

/// The reply text for an invocation by `author_id` in `channel_id`.
pub fn respond(
  invocation: &Invocation,
  settings: &Settings,
  author_id: u64,
  channel_id: u64,
  roll: &mut impl FnMut(u32) -> u32,
) -> String {
  let (default_sides, spoiler) = match invocation.command {
    Command::Help => return help_text(&settings.prefixes),
    Command::Roll => (settings.default_sides, false),
    Command::RollSpoiler => (settings.default_sides, true),
    Command::Ten => (10, false),
    Command::TenSpoiler => (10, true),
  };
  let options = RollOptions {
    default_sides,
    spoiler,
    d10_only: settings.d10_only_channels.contains(&channel_id),
  };
  let reply = dice::roll_reply(&format!("<@{author_id}>"), &invocation.args, options, roll);
  if invocation.yelling {
    format!("{YELL}{reply}")
  } else {
    reply
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn prefixes() -> Vec<String> {
    vec!["?".into(), ",".into()]
  }

  fn parsed(content: &str) -> Option<(Command, String, bool)> {
    parse(content, &prefixes(), Some(99)).map(|i| (i.command, i.args, i.yelling))
  }

  #[test]
  fn prefixes_and_mentions() {
    assert_eq!(
      parsed("?roll 2d6"),
      Some((Command::Roll, "2d6".into(), false))
    );
    assert_eq!(
      parsed(",dice 1d20 +2"),
      Some((Command::Roll, "1d20 +2".into(), false))
    );
    assert_eq!(parsed("? ten +1"), Some((Command::Ten, "+1".into(), false)));
    assert_eq!(
      parsed("<@99> rollsp 1d4"),
      Some((Command::RollSpoiler, "1d4".into(), false))
    );
    assert_eq!(
      parsed("<@!99> tensp"),
      Some((Command::TenSpoiler, String::new(), false))
    );
    assert_eq!(parsed("?help"), Some((Command::Help, String::new(), false)));
  }

  #[test]
  fn ignores_other_messages() {
    assert_eq!(parsed("roll 2d6"), None);
    assert_eq!(parsed("?what is this"), None);
    assert_eq!(parsed("<@12> roll"), None);
    assert_eq!(parsed("?commands"), None);
    assert_eq!(
      parse("?roll", &prefixes(), None).map(|i| i.command),
      Some(Command::Roll)
    );
  }

  #[test]
  fn command_names_are_case_insensitive_and_capitals_yell() {
    assert_eq!(
      parsed("?ROLL 1d6"),
      Some((Command::Roll, "1d6".into(), true))
    );
    assert_eq!(
      parsed("?Roll 1d6"),
      Some((Command::Roll, "1d6".into(), false))
    );
    assert_eq!(
      parsed("?TENSP"),
      Some((Command::TenSpoiler, String::new(), true))
    );
  }

  #[test]
  fn glued_shortcuts() {
    assert_eq!(parsed("?roll3"), Some((Command::Roll, "3".into(), false)));
    assert_eq!(
      parsed("?rollsp+2"),
      Some((Command::RollSpoiler, "+2".into(), false))
    );
    assert_eq!(parsed("?ten3-1"), Some((Command::Ten, "3-1".into(), false)));
    assert_eq!(
      parsed("?TENSP2"),
      Some((Command::TenSpoiler, "2".into(), true))
    );
    assert_eq!(parsed("?roll3 extra"), None);
  }

  fn settings() -> Settings {
    Settings {
      prefixes: prefixes(),
      default_sides: 20,
      d10_only_channels: HashSet::from([7]),
    }
  }

  fn reply(content: &str, channel: u64) -> String {
    let invocation = parse(content, &prefixes(), Some(99)).unwrap();
    respond(&invocation, &settings(), 5, channel, &mut |sides| sides)
  }

  #[test]
  fn replies() {
    assert_eq!(
      reply("?roll +1", 1),
      "<@5> requested 1d20+1 and rolled **21** (20, 1)."
    );
    assert_eq!(reply("?ten", 1), "<@5> requested 1d10 and rolled **10**.");
    assert_eq!(
      reply("?tensp", 1),
      "<@5> requested 1d10 and ||rolled **10**||."
    );
    assert_eq!(
      reply("?ROLL", 1),
      format!("{YELL}<@5> requested 1d20 and rolled **20**.")
    );
    assert!(reply("?help", 1).contains("`?roll EXPR…`"));
  }

  #[test]
  fn spoiler_roll_uses_the_default_die() {
    // rollsp used to roll d10s by mistake.
    assert_eq!(
      reply("?rollsp", 1),
      "<@5> requested 1d20 and ||rolled **20**||."
    );
  }

  #[test]
  fn d10_only_channel() {
    assert!(reply("?roll 1d20", 7).contains("You must roll a d10"));
    assert_eq!(
      reply("?ten 2", 7),
      "<@5> requested 1d10 1d10 and rolled **10**, **10**."
    );
  }
}
