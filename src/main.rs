#![allow(unused_imports)]
#![allow(clippy::write_with_newline)]
#![allow(clippy::len_zero)]

use std::convert::TryInto;
use lazy_static::lazy_static;
use rand::*;
use serenity::{
  async_trait,
  client::{bridge::gateway::ShardManager, *},
  framework::standard::{macros::*, *},
  model::{channel::*, event::*, gateway::*, id::*, user::User},
  prelude::*,
  utils::*,
  http::Http,
};
use std::{
  collections::{HashMap, HashSet},
  fmt::Write,
  process::{Command, Stdio},
  sync::Arc,
  env,
};
extern crate regex;
use regex::*;
use tokio::*;
use tokio::sync::*;

mod dice;
use dice::*;

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

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
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


#[hook]
async fn unknown_command(_ctx: &Context, _msg: &Message, _unknown_command_name: &str) {
  match starts_with_any(&_msg.content, &*PREFIXES) {
    Some(prefix) => {
      let snip = &_msg.content.to_lowercase()[prefix.len()..];
      if WONKY_ROLL.is_match(snip) {
        match dice(_ctx, _msg, Args::new(&_msg.content[prefix.len()+4..], &[Delimiter::Single(' ')])).await {
          Ok(_) => (),
          Err(why) => println!("{:?}", why)
        };
      }
      else if WONKY_TEN.is_match(snip) {
        match ten(_ctx, _msg, Args::new(&_msg.content[prefix.len()+3..], &[Delimiter::Single(' ')])).await {
          Ok(_) => (),
          Err(why) => println!("{:?}", why)
        };
      }
    },
    None => ()
  };
}

#[tokio::main]
async fn main() {
  // Configure the client with your Discord bot token in the environment.
  let token = env::var("DISCORD_TOKEN").expect(
    "Expected a token in the environment",
  );
  let userid_str = &::std::env::var("USER_ID").expect("Could not obtain USER_ID");
  let userid: UserId = UserId(userid_str.parse::<u64>().unwrap());
  let _prefixes = &::std::env::var("PREFIXES").unwrap_or_else(|_| "".to_string());

  let http = Http::new_with_token(&token);

  // We will fetch your bot's owners and id
  let (_owners, bot_id) = match http.get_current_application_info().await {
      Ok(info) => {
          let mut owners = HashSet::new();
          if let Some(team) = info.team {
              owners.insert(team.owner_user_id);
          } else {
              owners.insert(info.owner.id);
          }
          match http.get_current_user().await {
              Ok(bot_id) => (owners, bot_id.id),
              Err(why) => panic!("Could not access the bot id: {:?}", why),
          }
      },
      Err(why) => panic!("Could not access application info: {:?}", why),
  };

  let framework = StandardFramework::new()
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
  .bucket("help", |b| b.delay(30)).await
  .bucket("complicated", |b| b.delay(5).time_span(30).limit(2)).await
  .group(&GENERAL_GROUP)
  .help(&MY_HELP)
  .unrecognised_command(unknown_command);

  let mut client = Client::builder(&token)
      .event_handler(Handler)
      .framework(framework)
      .await
      .expect("Err creating client");

  {
      let mut data = client.data.write().await;
      data.insert::<CommandCounter>(HashMap::default());
      data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
  }

  if let Err(why) = client.start().await {
      println!("Client error: {:?}", why);
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

// This replaces the information that a user can pass
// a command-name as argument to gain specific information about it.
#[individual_command_tip =
"Hello! If you want more information about a specific command, just pass the command as argument."]
// Some arguments require a `{}` in order to replace it with contextual information.
// In this case our `{}` refers to a command's name.
#[command_not_found_text = "Could not find: `{}`."]
// Define the maximum Levenshtein-distance between a searched command-name
// and commands. If the distance is lower than or equal the set distance,
// it will be displayed as a suggestion.
// Setting the distance to 0 will disable suggestions.
#[max_levenshtein_distance(3)]
// When you use sub-groups, Serenity will use the `indention_prefix` to indicate
// how deeply an item is indented.
// The default value is "-", it will be changed to "+".
#[indention_prefix = "+"]
// On another note, you can set up the help-menu-filter-behaviour.
// Here are all possible settings shown on all possible options.
// First case is if a user lacks permissions for a command, we can hide the command.
#[lacking_permissions = "Hide"]
// If the user is nothing but lacking a certain role, we just display it hence our variant is `Nothing`.
#[lacking_role = "Nothing"]
// The last `enum`-variant is `Strike`, which ~~strikes~~ a command.
#[wrong_channel = "Strike"]
// Serenity will automatically analyse and generate a hint/tip explaining the possible
// cases of ~~strikethrough-commands~~, but only if
// `strikethrough_commands_tip_in_{dm, guild}` aren't specified.
// If you pass in a value, it will be displayed instead.
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

// Commands can be created via the attribute `#[command]` macro.
#[command]
// Options are passed via subsequent attributes.
// Make this command use the "complicated" bucket.
#[bucket = "complicated"]
async fn commands(ctx: &Context, msg: &Message) -> CommandResult {
    let mut contents = "Commands used:\n".to_string();

    let data = ctx.data.read().await;
    let counter = data.get::<CommandCounter>().expect("Expected CommandCounter in TypeMap.");

    for (k, v) in counter {
        writeln!(contents, "- {name}: {amount}", name=k, amount=v)?;
    }

    msg.channel_id.say(&ctx.http, &contents).await?;

    Ok(())
}

