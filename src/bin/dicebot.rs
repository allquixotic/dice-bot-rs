#![allow(unused_imports)]
#![allow(clippy::write_with_newline)]
#![allow(clippy::len_zero)]
//#![feature(const_fn)]

use std::convert::TryInto;
use lazy_static::lazy_static;
use dice_bot::{earthdawn::*, eote::*, shadowrun::*, *};
use randomize::*;
use serenity::{
  client::{bridge::gateway::ShardManager, *},
  framework::standard::{macros::*, *},
  model::{channel::*, event::*, gateway::*, id::*, user::User},
  prelude::*,
  utils::*,
};
use std::{
  collections::{HashMap, HashSet},
  fmt::Write,
  process::{Command, Stdio},
  sync::Arc,
};
extern crate regex;
use regex::*;

// A container type is created for inserting into the Client's `data`, which
// allows for data to be accessible across all events and framework commands, or
// anywhere else that has a copy of the `data` Arc.
struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
  type Value = Arc<Mutex<ShardManager>>;
}

struct CommandCounter;

impl TypeMapKey for CommandCounter {
  type Value = HashMap<String, u64>;
}

pub struct Handler;

impl EventHandler for Handler {
  fn ready(&self, _: Context, ready: Ready) {
    println!("{} is connected!", ready.user.name);
  }
}

#[group]
#[commands(commands, ddate, after_sundown, dice, troll, thaco, sigil_command, stat2e, champions)]
struct General;

lazy_static! {
  static ref DICE_MASSAGE: Regex = Regex::new(r"\s+(\+|-)").unwrap();
  static ref IMPLICIT_ROLL: Regex = Regex::new(r"^\s*(?:(\d*)((?:\+|-)?\d+)??)?$").unwrap();
  static ref WONKY_ROLL: Regex = Regex::new(r"^roll((\d*)(?:\+|-)?\d+)$").unwrap();
  static ref WONKY_TROLL: Regex = Regex::new(r"^troll((\d*)(?:\+|-)?\d+)$").unwrap();
  static ref DEFAULT_DICE: u32 = match ::std::env::var("DEFAULT_DICE") {
    Ok(num) => match num.parse::<u32>() {
      Ok(realnum) => realnum,
      Err(why) => panic!("DEFAULT_DICE environment variable is not a number. {:?}", why),
    },
    Err(_why2) => 20
  };
  static ref PREFIXES: Vec<String> = match ::std::env::var("PREFIXES") {
    Ok(prefixes_str) => {
      let mut v : Vec<String> = Vec::new();
      for prefix in prefixes_str.split_whitespace() {
        v.push(prefix.to_string());
      }
      v
    },
    Err(_) => vec!["?".to_string(), ",".to_string()]
  };
}

fn main() {
  just_seed_the_global_gen();
  let mut client = Client::new(
    &::std::env::var("DISCORD_TOKEN").expect("Could not obtain DISCORD_TOKEN"),
    Handler,
  )
  .expect("Could not create the client");

  {
    let mut data = client.data.write();
    data.insert::<CommandCounter>(HashMap::default());
    data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
  }

  // We will fetch your bot's id.
  let bot_id = match client.cache_and_http.http.get_current_application_info() {
    Ok(info) => info.id,
    Err(why) => panic!("Could not access application info: {:?}", why),
  };

  let userid_str = &::std::env::var("USER_ID").expect("Could not obtain USER_ID");
  let userid: UserId = UserId(userid_str.parse::<u64>().unwrap());
  let _prefixes = &::std::env::var("PREFIXES").unwrap_or_else(|_| "".to_string());

  client.with_framework(
    StandardFramework::new()
      .configure(|c| {
        c.allow_dm(true)
          .with_whitespace(WithWhiteSpace {
            prefixes: true,
            groups: true,
            commands: true,
          })
          .ignore_bots(true)
          .ignore_webhooks(true)
          .on_mention(Some(bot_id))
          .owners(vec![userid].into_iter().collect())
          .prefixes(&*PREFIXES)
          .no_dm_prefix(true)
          .delimiter(" ")
          .case_insensitivity(true)
      })
      .bucket("ddate", |b| b.delay(60))
      .bucket("help", |b| b.delay(30))
      .bucket("complicated", |b| b.delay(5).time_span(30).limit(2))
      .group(&GENERAL_GROUP)
      .group(&SHADOWRUN_GROUP)
      .group(&EOTE_GROUP)
      .group(&EARTHDAWN_GROUP)
      .help(&MY_HELP)
      .unrecognised_command(|ctx, msg, _cmd_name| {
        match starts_with_any(&msg.content, &*PREFIXES) {
          Some(prefix) => {
            let snip = &msg.content.to_lowercase()[prefix.len()..];
            if WONKY_ROLL.is_match(snip) {
              match dice(ctx, msg, Args::new(&msg.content[prefix.len()+4..], &[Delimiter::Single(' ')])) {
                Ok(_) => (),
                Err(why) => println!("{:?}", why)
              };
            }
            else if WONKY_TROLL.is_match(snip) {
              match troll(ctx, msg, Args::new(&msg.content[prefix.len()+5..], &[Delimiter::Single(' ')])) {
                Ok(_) => (),
                Err(why) => println!("{:?}", why)
              };
            }
          },
          None => ()
        };
      }),
  );

  if let Err(why) = client.start() {
    println!("Client::start error: {:?}", why);
  }
}

fn starts_with_any(haystack : &String, needles : &Vec<String>) -> Option<String> {
  for needle in needles {
    if haystack.starts_with(needle) {
      return Some(needle.to_string());
    }
  }
  return None;
}


#[help]
fn my_help(
  context: &mut Context,
  msg: &Message,
  args: Args,
  help_options: &'static HelpOptions,
  groups: &[&'static CommandGroup],
  owners: HashSet<UserId>,
) -> CommandResult {
  help_commands::with_embeds(context, msg, args, help_options, groups, owners)
}

// Commands can be created via the attribute `#[command]` macro.
#[command]
// Options are passed via subsequent attributes.
// Make this command use the "complicated" bucket.
#[bucket = "complicated"]
fn commands(ctx: &mut Context, msg: &Message) -> CommandResult {
  let mut contents = "Commands used:\n".to_string();

  let data = ctx.data.read();
  let counter = data
    .get::<CommandCounter>()
    .expect("Expected CommandCounter in ShareMap.");

  for (k, v) in counter {
    let _ = write!(contents, "- {name}: {amount}\n", name = k, amount = v);
  }

  if let Err(why) = msg.channel_id.say(&ctx.http, &contents) {
    println!("Error sending message: {:?}", why);
  }

  Ok(())
}

/// Opens a child process to check the `ddate` value.
fn ddate_process() -> Option<String> {
  String::from_utf8(
    Command::new("ddate")
      .stdout(Stdio::piped())
      .spawn()
      .ok()?
      .wait_with_output()
      .ok()?
      .stdout,
  )
  .ok()
}

#[command]
#[description = "https://en.wikipedia.org/wiki/Discordian_calendar"]
#[bucket = "ddate"]
fn ddate(_ctx: &mut Context, msg: &Message, _: Args) -> CommandResult {
  if let Some(date) = ddate_process() {
    if let Err(why) = msg.channel_id.say(&_ctx.http, date) {
      println!("Error sending message: {:?}", why);
    }
  }
  Ok(())
}

#[command]
#[aliases("as")]
#[description = "Rolls After Sundown style"]
#[usage = "DICE [...]"]
fn after_sundown(_ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
  let gen: &mut PCG32 = &mut global_gen();
  let mut output = String::new();
  for dice_count in args
    .rest()
    .split_whitespace()
    .flat_map(basic_sum_str)
    .take(10)
  {
    let dice_count = dice_count.max(0).min(5_000) as u32;
    if dice_count > 0 {
      let mut hits = 0;
      const DICE_REPORT_MAXIMUM: u32 = 30;
      let mut dice_record = String::with_capacity(DICE_REPORT_MAXIMUM as usize * 2 + 20);
      dice_record.push_str(" `(");
      for _ in 0..dice_count {
        let roll = D6.sample(gen);
        if roll >= 5 {
          hits += 1;
        }
        if dice_count < DICE_REPORT_MAXIMUM {
          dice_record.push((b'0' + roll as u8) as char);
          dice_record.push(',');
        }
      }
      dice_record.pop();
      // I have ABSOLUTELY no idea why we need to put this extra space in here,
      // but we do and that makes the output correct.
      dice_record.push_str(")`");
      let s_for_hits = if hits != 1 { "s" } else { "" };
      let dice_report_output = if dice_count < DICE_REPORT_MAXIMUM {
        &dice_record
      } else {
        ""
      };
      output.push_str(&format!(
        "Rolled {} dice: {} hit{}{}",
        dice_count, hits, s_for_hits, dice_report_output
      ));
    } else {
      let output = format!("No dice to roll!");
      if let Err(why) = msg.channel_id.say(&_ctx.http, output) {
        println!("Error sending message: {:?}", why);
      }
    }
    output.push('\n');
  }
  output.pop();
  if output.len() > 0 {
    if let Err(why) = msg.channel_id.say(&_ctx.http, output) {
      println!("Error sending message: {:?}", why);
    }
  }
  Ok(())
}

fn dice_get_string(gen: &mut PCG32, author: &User, args: &str, ten: bool) -> String {
  let mut args_not_lower : String = args.to_string();
  //println!("{}", args);
  if IMPLICIT_ROLL.is_match(args) {
    let caps = IMPLICIT_ROLL.captures(args).unwrap(); 
    let mut dd : u32 = *DEFAULT_DICE;
    if ten {
      dd = 10;
    }
    let mut first_num = match caps.get(1) {
      Some(cap) => cap.as_str(),
      None => "1"
    };
    let mut first_num_num : u32 = match first_num.parse::<u32>() {
      Ok(p) => {
        if p < 1 {
          first_num = "1";
          1
        }
        else {
          p
        }
      },
      Err(_) => {
        first_num = "1";
        1
      }
    };
    let plus_num = match caps.get(2) {
      Some(cap) => cap.as_str(),
      None => ""
    };
    //println!("first_num: {}, first_num_num: {}, plus_num: {}", first_num, first_num_num, plus_num);
    if first_num_num > 1 {
      let temparg = format!("1d{}{} ", dd, plus_num);
      if first_num_num > 50 {
        first_num_num = 50;
      }
      args_not_lower = temparg.repeat(first_num_num.try_into().unwrap()).trim().to_string();
    }
    else {
      args_not_lower = format!("{}d{}{}", first_num, dd, plus_num);
    }
  }
  let argslower = DICE_MASSAGE.replace_all(&args_not_lower.to_lowercase(), "$1").to_string();
  let mut output;
  let mut mb = MessageBuilder::new();
  let mut parsed_string;
  let mut first_iter = true;
  'exprloop: for dice_expression_str in argslower.split_whitespace().take(50) {
    let mut vec = Vec::new();
    let plus_only_form = dice_expression_str.replace("-", "+-");
    let mut total: i32 = 0;
    let mut sub_expressions = vec![];
    for sub_expression in plus_only_form.split('+').take(70) {
      if sub_expression.len() == 0 {
        continue;
      }
      let mut d_iter = sub_expression.split('d');
      let num_dice: i32 = match d_iter.next() {
        Some(num_dice_str) => {
          if num_dice_str.len() > 0 {
            match num_dice_str.parse::<i32>() {
              Ok(num) => num.max(-5_000).min(5_000),
              Err(_) => {
                //msg.react(ReactionType::Unicode(EMOJI_QUESTION.to_string())).ok();
                continue 'exprloop;
              }
            }
          } else {
            1
          }
        }
        None => {
          //msg.react(ReactionType::Unicode(EMOJI_QUESTION.to_string())).ok();
          continue 'exprloop;
        }
      };
      let num_sides: u32 = match d_iter.next() {
        Some(num_sides_str) => {
          match num_sides_str.parse::<u32>() {
            Ok(num) => num.min(4_000_000),
            Err(_) => {
              //msg.react(ReactionType::Unicode(EMOJI_QUESTION.to_string())).ok();
              continue 'exprloop;
            }
          }
        }
        None => 1,
      };
      if d_iter.next().is_some() {
        //msg.react(ReactionType::Unicode(EMOJI_QUESTION.to_string())).ok();
        continue 'exprloop;
      }
      if num_sides == 0 {
        // do nothing with 0-sided dice
      } else if num_sides == 1 {
        vec.push(num_dice);
        total += num_dice;
        sub_expressions.push(format!("{}", num_dice));
      } else {
        let range = match num_sides {
          4 => D4,
          6 => D6,
          8 => D8,
          10 => D10,
          12 => D12,
          20 => D20,
          _ => RandRangeU32::new(1, num_sides),
        };
        if num_dice > 0 {
          for _ in 0..num_dice {
            let pf = range.sample(gen) as i32;
            vec.push(pf);
            total += pf;
          }
          sub_expressions.push(format!("{}d{}", num_dice, num_sides));
        } else if num_dice < 0 {
          for _ in 0..num_dice.abs() {
            let pq = range.sample(gen) as i32;
            vec.push(pq);
            total -= pq;
          }
          sub_expressions.push(format!("{}d{}", num_dice, num_sides));
        }
        // do nothing if num_dice == 0
      }
    }
    if sub_expressions.len() > 0 {
      parsed_string = sub_expressions[0].clone();
      for sub_expression in sub_expressions.into_iter().skip(1) {
        if sub_expression.chars().nth(0) == Some('-') {
          parsed_string.push_str(&sub_expression);
        } else {
          parsed_string.push('+');
          parsed_string.push_str(&sub_expression);
        }
      }
      let veq = format!("{:?}", vec).replace("[", "(").replace("]", ")");
      if first_iter {
        mb.mention(author)
        .push(" requested ")
        .push(&args_not_lower)
        .push( " and rolled ")
        .push_bold(total);
      }
      else {
        mb.push(", ")
        .push_bold(total);
      }
      
      if vec.len() > 1 {
        mb.push(" ")
        .push(veq);
      }
    } 
    first_iter = false;
  }
  if mb.build().len() > 0 {
    mb.push(".");
  }
  output = mb.build();
  if output.len() <= 0 {
    mb = MessageBuilder::new();
    mb.mention(author)
    .push(" Unable to process the supplied dice expression because I didn't understand the dice syntax you supplied.");
    output = mb.build();
  }
  return output;
}

#[command]
#[aliases("roll", "dice")]
#[description = "Rolls a standard dice expression"]
#[usage = "EXPRESSION [...]"]
fn dice(_ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
  let gen: &mut PCG32 = &mut global_gen();
  let mut output = dice_get_string(gen, &msg.author, args.rest(), false);

  let yelling = msg.content.contains("ROLL");
  if yelling {
    output = "Wow, okay! Is your caps lock on, or are you mad at me? :( ".to_owned() + &output;
  }

  if let Err(why) = msg.channel_id.say(&_ctx.http, output) {
    println!("Error sending message: {:?}", why);
    let built : String = "ERROR: Failed to send you a valid response, either because the response would be too long, or the Discord server didn't like it for some other reason. Please try again.".to_string();
    if let Err(why2) = msg.channel_id.say(&_ctx.http, built) {
      println!("Error sending message: {:?}", why2);
    }
  }
  Ok(())
}

#[command]
#[aliases("troll")]
#[description = "Rolls a standard dice expression assuming d10"]
#[usage = "EXPRESSION [...]"]
fn troll(_ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
  let gen: &mut PCG32 = &mut global_gen();
  let mut output = dice_get_string(gen, &msg.author, args.rest(), true);

  let yelling = msg.content.contains("TROLL");
  if yelling {
    output = "Wow, okay! Is your caps lock on, or are you mad at me? :( ".to_owned() + &output;
  }

  if let Err(why) = msg.channel_id.say(&_ctx.http, output) {
    println!("Error sending message: {:?}", why);
    let built : String = "ERROR: Failed to send you a valid response, either because the response would be too long, or the Discord server didn't like it for some other reason. Please try again.".to_string();
    if let Err(why2) = msg.channel_id.say(&_ctx.http, built) {
      println!("Error sending message: {:?}", why2);
    }
  }
  Ok(())
}

#[command]
#[description = "Does a THACO attack roll"]
#[usage = "THACO [...]"]
#[aliases("thaco", "taco")]
fn thaco(_ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
  let gen: &mut PCG32 = &mut global_gen();
  let mut output = String::new();
  for thaco_value in args
    .rest()
    .split_whitespace()
    .flat_map(basic_sum_str)
    .take(20)
  {
    let roll = D20.sample(gen) as i32;
    output.push_str(&format!(
      "THACO {}: Rolled {}, Hits AC {} or greater.\n",
      thaco_value,
      roll,
      thaco_value - roll
    ));
  }
  output.pop();
  if output.len() > 0 {
    if let Err(why) = msg.channel_id.say(&_ctx.http, output) {
      println!("Error sending message: {:?}", why);
    }
  }
  Ok(())
}

#[command]
#[description = "It does a mystery thing that Sigil decided upon"]
#[aliases("sigil")]
#[usage = "BASIC_SUM_STRING [...]"]
fn sigil_command(_ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
  let gen: &mut PCG32 = &mut global_gen();
  let mut output = String::new();
  let terms: Vec<i32> = args
    .rest()
    .split_whitespace()
    .filter_map(basic_sum_str)
    .collect();
  for term in terms {
    let x = term.abs();
    if x > 0 {
      let mut total: i32 = 0;
      for _ in 0..x {
        total += D6.sample(gen) as i32;
        total -= D6.sample(gen) as i32;
      }
      output.push_str(&format!("Rolling Sigil {}: {}\n", x, total.abs()));
    } else {
      output.push_str(&format!("Rolling Sigil {}: 0\n", x));
    }
  }
  output.pop();
  if output.len() > 0 {
    if let Err(why) = msg.channel_id.say(&_ctx.http, output) {
      println!("Error sending message: {:?}", why);
    }
  } else if let Err(why) = msg.channel_id.say(&_ctx.http, "usage: sigil NUMBER") {
    println!("Error sending message: {:?}", why);
  }

  Ok(())
}

#[command]
#[description = "Rolls a 2e stat array"]
#[aliases("stat2e")]
fn stat2e(_ctx: &mut Context, msg: &Message, _args: Args) -> CommandResult {
  let gen: &mut PCG32 = &mut global_gen();
  let mut output = String::new();
  let roll =
    |gen: &mut PCG32| 4 + D4.sample(gen) + D4.sample(gen) + D4.sample(gen) + D4.sample(gen);
  output.push_str(&format!("Str: {}\n", roll(gen)));
  output.push_str(&format!("Dex: {}\n", roll(gen)));
  output.push_str(&format!("Con: {}\n", roll(gen)));
  output.push_str(&format!("Int: {}\n", roll(gen)));
  output.push_str(&format!("Wis: {}\n", roll(gen)));
  output.push_str(&format!("Cha: {}\n", roll(gen)));
  output.pop();
  if let Err(why) = msg.channel_id.say(&_ctx.http, output) {
    println!("Error sending message: {:?}", why);
  }
  Ok(())
}

#[command]
#[aliases("champ")]
#[description = "Rolls a Champions roll"]
#[usage = "EXPRESSION [...]"]
fn champions(_ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
  let gen: &mut PCG32 = &mut global_gen();
  let mut output = String::new();
  let terms: Vec<i32> = args
    .rest()
    .split_whitespace()
    .filter_map(basic_sum_str)
    .collect();
  for term in terms {
    let mut rolls = [0; 3];
    for roll_mut in rolls.iter_mut() {
      *roll_mut = D6.sample(gen) as i32;
    }
    output.push_str(&format!(
      "Rolling Champions {}: {}, [{},{},{}]\n",
      term,
      if rolls.iter().cloned().sum::<i32>() < term {
        "Success"
      } else {
        "Failure"
      },
      rolls[0],
      rolls[1],
      rolls[2]
    ));
  }
  output.pop();
  if output.len() > 0 {
    if let Err(why) = msg.channel_id.say(&_ctx.http, output) {
      println!("Error sending message: {:?}", why);
    }
  } else if let Err(why) = msg.channel_id.say(&_ctx.http, "usage: sigil NUMBER") {
    println!("Error sending message: {:?}", why);
  }
  Ok(())
}

//XXX: My tests don't work until this is merged: https://github.com/serenity-rs/serenity/pull/778

// #[cfg(test)]
// mod tests {
//   use super::*;

//   fn rng0() -> PCG32 {
//     return PCG32::seed(0,0);
//   }

//   fn test_get_num(index: u32, die: u32) -> u32 {
//     let mut rng = rng0();
//     let rr = RandRangeU32::new(1, die);
//     let mut retval : u32 = 0;
//     for _i in 0..index+1 {
//       retval = rr.sample(&mut rng);
//     }
//     return retval;
//   }
  
//   #[test]
//   fn dice_str_nonsense() {
//     let gen = &mut rng0();
//     let user : User = User::default();
//     let args = " Unable to process the supplied dice expression because I didn't understand the dice syntax you supplied.";
//     assert_eq!(dice_get_string(gen, &user, args, false), "<@210> Unable to process the supplied dice expression because I didn't understand the dice syntax you supplied.");
//   }
  
//   #[test]
//   fn dice_str_torture() {
//     let tests: HashMap<&str, String> = 
//     [
//       ("1d10", format!("**{}**", test_get_num(0, 10))),
//       ("1d10+1", format!("**{}** ({}, {})", test_get_num(0, 10)+1, test_get_num(0, 10), 1)),
//       ("1d10+1 1D10", format!("**{}** ({}, {}), **{}**", test_get_num(0, 10)+1, test_get_num(0, 10), 1, test_get_num(1, 10))),
//       ("1d20+1", format!("**{}** ({}, {})", test_get_num(0, 20)+1, test_get_num(0, 20), 1)),
//       ("1d20+2 1d20+3", format!("**{}** ({}, {}), **{}** ({}, {})", test_get_num(0, 20)+2, test_get_num(0, 20), 2, test_get_num(1, 20)+3, test_get_num(1, 20), 3)),
//       ("1d20 +2 1d20  +3", format!("**{}** ({}, {}), **{}** ({}, {})", test_get_num(0, 20)+2, test_get_num(0, 20), 2, test_get_num(1, 20)+3, test_get_num(1, 20), 3)),
//       ("1D20 +2 1D20  +3", format!("**{}** ({}, {}), **{}** ({}, {})", test_get_num(0, 20)+2, test_get_num(0, 20), 2, test_get_num(1, 20)+3, test_get_num(1, 20), 3)),
//     ]
//     .iter().cloned().collect();
//     let user : User = User::default();
//     for (args, retnum) in tests {
//       let retval = format!("<@{}> requested {} and rolled {}.", user.id, args, retnum);
//       let gen = &mut rng0();
//       assert_eq!(dice_get_string(gen, &user, args, false), retval);
//     }
//   }

//   #[test]
//   fn dice_str_default() {
//     let roll1 : u32 = test_get_num(0, *DEFAULT_DICE);
//     let roll2 : u32 = test_get_num(1, *DEFAULT_DICE);
//     let gen = &mut rng0();
//     let user : User = User::default();
//     let mut retval = format!("<@{}> requested {} and rolled **{}** ({}, {}).", user.id, "1d20+2", roll1+2, roll1, 2);
//     assert_eq!(dice_get_string(&mut rng0(), &user, "+2", false), retval);

//     retval = format!("<@{}> requested {} and rolled **{}**.", user.id, "1d20", roll1);
//     assert_eq!(dice_get_string(&mut rng0(), &user, "", false), retval);

//     retval = format!("<@{}> requested {} and rolled **{}**, **{}**.", user.id, "1d20 1d20", roll1, roll2);
//     assert_eq!(dice_get_string(&mut rng0(), &user, "2", false), retval);
//   }

//   #[test]
//   fn troll_default() {
//     let roll1 : u32 = test_get_num(0, 10);
//       let roll2 : u32 = test_get_num(1, 10);
//       let gen = &mut rng0();
//       let user : User = User::default();
//       let mut retval = format!("<@{}> requested {} and rolled **{}** ({}, {}).", user.id, "1d10+2", roll1+2, roll1, 2);
//       assert_eq!(dice_get_string(gen, &user, "+2", true), retval);
//       retval = format!("<@{}> requested {} and rolled **{}**.", user.id, "1d10", roll2);
//       assert_eq!(dice_get_string(gen, &user, "", true), retval);
//   }
// }
