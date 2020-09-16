#![allow(unused_imports)]
#![allow(clippy::write_with_newline)]
#![allow(clippy::len_zero)]

use std::convert::TryInto;
use lazy_static::lazy_static;
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

mod dice;
use dice::*;

mod ranges;

mod global_gen;

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

lazy_static! {
  static ref WONKY_ROLL: Regex = Regex::new(r"^roll((\d*)(?:\+|-)?\d+)$").unwrap();
  static ref WONKY_TEN: Regex = Regex::new(r"^ten((\d*)(?:\+|-)?\d+)$").unwrap();
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
      .bucket("help", |b| b.delay(30))
      .bucket("complicated", |b| b.delay(5).time_span(30).limit(2))
      .group(&GENERAL_GROUP)
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
            else if WONKY_TEN.is_match(snip) {
              match ten(ctx, msg, Args::new(&msg.content[prefix.len()+3..], &[Delimiter::Single(' ')])) {
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

