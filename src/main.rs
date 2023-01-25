use std::env;

use anyhow::Result;
use config::Config;
use handler::Handler;
use persistance::Storage;
use serenity::{framework::standard::StandardFramework, prelude::*};

mod command;
mod config;
mod handler;
mod interaction;
mod persistance;
mod play;
mod utils;

const CONFIG_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/config.toml");

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::new(CONFIG_FILE);
    let save_folder = config
        .get_string("SAVE_FOLDER")
        .expect("missing save folder");
    let database = Storage::new(save_folder)?;

    let framework = StandardFramework::new();

    // Login with a bot token from the environment
    let token = config
        .get_string("DISCORD_TOKEN")
        .expect("missing discord token");
    let intents = GatewayIntents::non_privileged();
    let mut client = Client::builder(token, intents)
        .event_handler(Handler {
            storage: Mutex::new(database),
        })
        .framework(framework)
        .await?;
    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
        Err(why)?
    } else {
        Ok(())
    }
}
