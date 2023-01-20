use serenity::{
    model::prelude::{
        interaction::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
        },
        ReactionType,
    },
    prelude::Context,
};

use crate::Handler;

pub async fn text_interaction(text: &str, ctx: &Context, command: &ApplicationCommandInteraction) {
    if let Err(why) = command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message
                        .embed(|embed| embed.title("Action").description(text))
                        .ephemeral(false)
                })
        })
        .await
    {
        println!("Cannot respond to slash command: {}", why);
    }
}

pub async fn increment_interaction(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) {
    let database = handler.database.lock().await;
    database.increment_count().unwrap();
    let count = database.get_count().unwrap();
    let message = format!("Count is now {count}");

    text_interaction(&message, ctx, command).await
}

pub async fn react_interaction(
    reaction_type: impl Into<ReactionType>,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) {
    if let Ok(message) = command.get_interaction_response(&ctx.http).await {
        if let Err(why) = message.react(&ctx.http, reaction_type).await {
            println!("Cannot react to slash command: {}", why);
        };
    };
}
