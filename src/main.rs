use std::env;

use anyhow::Result;
use command::{IncrementCommand, UploadStoryCommand};
use config::Config;
use persistance::Database;
use serenity::async_trait;
use serenity::framework::standard::StandardFramework;
use serenity::model::prelude::command::Command;
use serenity::model::prelude::interaction::Interaction;
use serenity::model::prelude::{Reaction, Ready};
use serenity::prelude::*;

use crate::command::{PingCommand, PongCommand, SlashCommand, SlashCommandCreator};
use crate::interaction::{increment_interaction, react_interaction, text_interaction};

mod command;
mod config;
mod interaction;
mod persistance;

const CONFIG_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/config.toml");

pub struct Handler {
    database: Mutex<Database>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            println!("Received command interaction: {:#?}", command.data.kind);
            match command.data.name.as_str() {
                PingCommand::NAME => {
                    text_interaction("Pong!", &ctx, &command).await;
                }
                PongCommand::NAME => {
                    text_interaction("Ping!", &ctx, &command).await;
                }
                IncrementCommand::NAME => {
                    increment_interaction(self, &ctx, &command).await;
                    react_interaction('⏰', &ctx, &command).await;
                }
                UploadStoryCommand::NAME => {
                    text_interaction("Trying to upload, I see!", &ctx, &command).await;
                }
                _ => {
                    println!("Not implemented :(");
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        Command::set_global_application_commands(&ctx.http, |commands| {
            commands
                .create_slash_command::<PingCommand>()
                .create_slash_command::<PongCommand>()
                .create_slash_command::<IncrementCommand>()
                .create_slash_command::<UploadStoryCommand>()
        })
        .await
        .unwrap();
    }

    async fn reaction_add(&self, _ctx: Context, add_reaction: Reaction) {
        println!("Reaction added: {:#?}", add_reaction.emoji);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::new(CONFIG_FILE);
    let sql_path = config.get_string("DATABASE_URL").expect("database");
    let database = Database::new(sql_path)?;

    let framework = StandardFramework::new();

    // Login with a bot token from the environment
    let token = config.get_string("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged();
    let mut client = Client::builder(token, intents)
        .event_handler(Handler {
            database: Mutex::new(database),
        })
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
        Err(why)?
    } else {
        Ok(())
    }
}
