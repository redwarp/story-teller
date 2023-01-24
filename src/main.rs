use std::env;

use anyhow::Result;
use command::{IncrementCommand, UploadStoryCommand};
use config::Config;
use interaction::{actual_deletion, delete_story_interaction, DELETE_STORY_MENU};
use persistance::Storage;
use play::{
    actual_start, next_chapter, play_story_interaction, stop_story_interaction, PICK_NEXT_PASSAGE,
    START_STORY_MENU,
};
use serenity::async_trait;
use serenity::framework::standard::StandardFramework;
use serenity::model::prelude::command::Command;
use serenity::model::prelude::interaction::Interaction;
use serenity::model::prelude::{Reaction, Ready};
use serenity::prelude::*;

use crate::command::{
    DeleteStoryCommand, PingCommand, PlayCommand, SlashCommand, SlashCommandCreator, StopCommand,
};
use crate::interaction::{
    increment_interaction, react_interaction, text_interaction, upload_story_interaction,
};

mod command;
mod config;
mod interaction;
mod persistance;
mod play;
mod utils;

const CONFIG_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/config.toml");

pub struct Handler {
    storage: Mutex<Storage<String>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            match command.data.name.as_str() {
                PingCommand::NAME => {
                    text_interaction("Pong!", &ctx, &command).await;
                }
                IncrementCommand::NAME => {
                    increment_interaction(self, &ctx, &command).await;
                    react_interaction('⏰', &ctx, &command).await;
                }
                UploadStoryCommand::NAME => {
                    upload_story_interaction(self, &ctx, &command).await;
                }
                DeleteStoryCommand::NAME => {
                    delete_story_interaction(self, &ctx, &command).await;
                }
                PlayCommand::NAME => {
                    play_story_interaction(self, &ctx, &command).await;
                }
                StopCommand::NAME => {
                    stop_story_interaction(self, &ctx, &command).await;
                }
                rest => {
                    println!("Command {rest} not implemented :(");
                    text_interaction(
                        format!("Command `{rest}` not implemented :("),
                        &ctx,
                        &command,
                    )
                    .await;
                }
            }
        } else if let Interaction::MessageComponent(message_component) = interaction {
            match message_component.data.custom_id.as_str() {
                DELETE_STORY_MENU => actual_deletion(self, &ctx, &message_component).await,
                START_STORY_MENU => actual_start(self, &ctx, &message_component).await,
                PICK_NEXT_PASSAGE => next_chapter(self, &ctx, &message_component).await,
                other => {
                    println!("Message component {other}");
                }
            }
        } else {
            println!("Something happened");
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        Command::set_global_application_commands(&ctx.http, |commands| {
            commands
                .create_slash_command::<PingCommand>()
                .create_slash_command::<IncrementCommand>()
                .create_slash_command::<UploadStoryCommand>()
                .create_slash_command::<DeleteStoryCommand>()
                .create_slash_command::<PlayCommand>()
                .create_slash_command::<StopCommand>()
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
    let save_folder = config.get_string("SAVE_FOLDER").expect("database");
    let database = Storage::new(save_folder)?;

    let framework = StandardFramework::new();

    // Login with a bot token from the environment
    let token = config.get_string("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged();
    let mut client = Client::builder(token, intents)
        .event_handler(Handler {
            storage: Mutex::new(database),
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
