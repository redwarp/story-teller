use serenity::{
    model::prelude::{
        command::CommandOptionType,
        interaction::{
            application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
            InteractionResponseType,
        },
        Attachment, ReactionType,
    },
    prelude::Context,
};

use crate::{utils::verify_story, Handler};

pub async fn text_interaction<T: ToString>(
    text: T,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) {
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

pub async fn upload_story_interaction(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) {
    if let Some(attachment) = command
        .data
        .options
        .iter()
        .find(|option| option.kind == CommandOptionType::Attachment)
        .and_then(|option| match &option.resolved {
            Some(CommandDataOptionValue::Attachment(attachment)) => Some(attachment),
            _ => None,
        })
    {
        if let Ok(content) = fetch_attachment(attachment).await {
            if verify_story(&content) {
                let database = handler.database.lock().await;
                let answer = match database.save_story(&content) {
                    Ok(_) => format!("Successfully uploaded {}", attachment.filename),
                    Err(_) => format!(
                        "Error while uploading {}, try again later.",
                        attachment.filename
                    ),
                };
                text_interaction(answer, ctx, command).await;
            } else {
                text_interaction(
                    format!("{} is not a valid story", attachment.filename),
                    ctx,
                    command,
                )
                .await;
            }
        } else {
            text_interaction(
                format!("Couldn't download {}", attachment.filename),
                ctx,
                command,
            )
            .await;
        }
    } else {
        text_interaction("No attachment found", ctx, command).await;
    }
}

async fn fetch_attachment(attachment: &Attachment) -> Result<String, reqwest::Error> {
    match reqwest::get(&attachment.url).await {
        Ok(response) => response.text().await,
        Err(e) => Err(e),
    }
}
