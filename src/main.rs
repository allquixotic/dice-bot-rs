//! Discord dice bot. Configuration comes from the environment (see .env.example).

mod commands;
mod dice;

use std::{
  collections::HashSet,
  env,
  sync::atomic::{AtomicU64, Ordering},
};

use rand::RngExt;
use serenity::{
  async_trait,
  model::{channel::Message, event::ResumedEvent, gateway::Ready},
  prelude::*,
};

use commands::Settings;

const SEND_FAILED: &str = "ERROR: Failed to send you a valid response, either because the response would be too long, \
                           or the Discord server didn't like it for some other reason. Please try again.";

fn settings_from_env() -> Result<Settings, String> {
  let prefixes = match env::var("PREFIXES") {
    Ok(text) => text.split_whitespace().map(str::to_string).collect(),
    Err(_) => vec!["?".to_string(), ",".to_string()],
  };
  let default_sides = match env::var("DEFAULT_DICE") {
    Ok(text) => text
      .trim()
      .parse()
      .map_err(|_| format!("DEFAULT_DICE must be a whole number, got {text:?}"))?,
    Err(_) => 20,
  };
  let d10_only_channels = env::var("D10_ONLY_ROLLS")
    .unwrap_or_default()
    .split_whitespace()
    .map(|id| {
      id.parse()
        .map_err(|_| format!("D10_ONLY_ROLLS must list channel IDs, got {id:?}"))
    })
    .collect::<Result<HashSet<u64>, String>>()?;
  Ok(Settings {
    prefixes,
    default_sides,
    d10_only_channels,
  })
}

struct Handler {
  settings: Settings,
  bot_id: AtomicU64,
}

#[async_trait]
impl EventHandler for Handler {
  async fn message(&self, ctx: Context, msg: Message) {
    if msg.author.bot || msg.webhook_id.is_some() {
      return;
    }
    let bot_id = Some(self.bot_id.load(Ordering::Relaxed)).filter(|&id| id != 0);
    let Some(invocation) = commands::parse(&msg.content, &self.settings.prefixes, bot_id) else {
      return;
    };
    let reply = {
      let mut rng = rand::rng();
      commands::respond(
        &invocation,
        &self.settings,
        msg.author.id.get(),
        msg.channel_id.get(),
        &mut |sides| rng.random_range(1..=sides),
      )
    };
    if let Err(error) = msg.channel_id.say(&ctx.http, &reply).await {
      eprintln!("Error sending reply in channel {}: {error}", msg.channel_id);
      if let Err(error) = msg.channel_id.say(&ctx.http, SEND_FAILED).await {
        eprintln!(
          "Error sending failure notice in channel {}: {error}",
          msg.channel_id
        );
      }
    }
  }

  async fn ready(&self, _ctx: Context, ready: Ready) {
    self.bot_id.store(ready.user.id.get(), Ordering::Relaxed);
    println!(
      "{} is connected! ({} guilds)",
      ready.user.name,
      ready.guilds.len()
    );
  }

  async fn resume(&self, _ctx: Context, _event: ResumedEvent) {
    println!("Gateway session resumed.");
  }
}

#[tokio::main]
async fn main() {
  let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set (see .env.example)");
  let settings = settings_from_env().unwrap_or_else(|error| panic!("{error}"));
  let intents =
    GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
  let handler = Handler {
    settings,
    bot_id: AtomicU64::new(0),
  };
  let mut client = Client::builder(&token, intents)
    .event_handler(handler)
    .await
    .expect("Error creating client");
  if let Err(error) = client.start().await {
    eprintln!("Client error: {error}");
    std::process::exit(1);
  }
}
