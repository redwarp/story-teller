use std::env;

use anyhow::Result;
use persistance::Database;
use serenity::async_trait;
use serenity::framework::standard::StandardFramework;
use serenity::model::prelude::command::Command;
use serenity::model::prelude::interaction::{Interaction, InteractionResponseType};
use serenity::model::prelude::{Reaction, Ready};
use serenity::prelude::*;

mod persistance;

struct Handler {
    database: Mutex<Database>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            println!("Received command interaction: {:#?}", command.data.kind);
            let content = match command.data.name.as_str() {
                "ping" => "Pong!".to_string(),
                "pong" => "Ping!".to_string(),
                "increment" => {
                    let database = self.database.lock().await;
                    database.increment_count().unwrap();
                    let count = database.get_count().unwrap();
                    format!("Count is now {count}")
                }
                "upload" => "Trying to upload I see!".to_string(),
                _ => "not implemented :(".to_string(),
            };

            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| {
                            message
                                .embed(|embed| embed.title("Action").description(content))
                                .ephemeral(false)
                        })
                })
                .await
            {
                println!("Cannot respond to slash command: {}", why);
            }

            if let Ok(message) = command.get_interaction_response(&ctx.http).await {
                if let Err(why) = message.react(&ctx.http, '👍').await {
                    println!("Cannot react to slash command: {}", why);
                };
            };
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        Command::set_global_application_commands(&ctx.http, |commands| {
            commands
                .create_application_command(|command| {
                    command.name("ping").description("Pings the bot")
                })
                .create_application_command(|command| {
                    command.name("pong").description("Pongs the bot")
                })
                .create_application_command(|command| {
                    command
                        .name("increment")
                        .description("Increments a counter")
                })
                .create_application_command(|command| {
                    command
                        .name("upload")
                        .description("Upload a story")
                        .create_option(|option| {
                            option.kind(
                                serenity::model::prelude::command::CommandOptionType::Attachment,
                            ).name("file").required(true).description("The file to upload")
                        })
                })
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
    let sql_path = env::var("DATABASE_URL").expect("database");
    let database = Database::new(sql_path)?;

    let framework = StandardFramework::new();

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("token");
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
