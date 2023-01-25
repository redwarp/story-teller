use anyhow::{anyhow, Result};
use reqwest::Client;
use serenity::{
    model::prelude::{
        command::CommandOptionType,
        interaction::{
            application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
            message_component::MessageComponentInteraction,
            InteractionResponseType,
        },
        Attachment, ReactionType,
    },
    prelude::Context,
};

use crate::{persistance::SaveStory, utils::story_title, Handler};

pub const DELETE_STORY_MENU: &str = "delete_story_menu";

pub async fn text_interaction<T: ToString>(
    text: T,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) {
    if let Err(why) = command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|response| {
                    response
                        .embed(|embed| embed.title("Action").description(text))
                        .ephemeral(true)
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
    let database = handler.storage.lock().await;
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
    let guild_id = if let Some(guild_id) = command.guild_id {
        guild_id.to_string()
    } else {
        return;
    };

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
            let story_title = story_title(&content);
            if story_title.is_some() {
                let database = handler.storage.lock().await;
                let answer = match database.save_story(&guild_id, &content) {
                    Ok(save_story) => match save_story {
                        SaveStory::New => {
                            format!(
                                "Successfully uploaded `{}`, creating story `{}`",
                                attachment.filename,
                                story_title.unwrap()
                            )
                        }
                        SaveStory::Update => format!(
                            "Successfully uploaded `{}`, updating existing story `{}`",
                            attachment.filename,
                            story_title.unwrap()
                        ),
                    },
                    Err(_) => format!(
                        "Error while uploading `{}`, try again later.",
                        attachment.filename
                    ),
                };
                text_interaction(answer, ctx, command).await;
            } else {
                text_interaction(
                    format!("`{}` is not a valid story", attachment.filename),
                    ctx,
                    command,
                )
                .await;
            }
        } else {
            text_interaction(
                format!("Couldn't download `{}`", attachment.filename),
                ctx,
                command,
            )
            .await;
        }
    } else {
        text_interaction("No attachment found", ctx, command).await;
    }
}

pub async fn delete_story_interaction(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) {
    let guild_id = if let Some(guild_id) = command.guild_id {
        guild_id.to_string()
    } else {
        return;
    };

    let text = "Please select the story you want to delete:";
    let database = handler.storage.lock().await;
    let all_stories = database.list_guild_stories(&guild_id);

    let stories = if let Ok(stories) = all_stories {
        stories
    } else {
        text_interaction(
            "We couldn't list the stories, try again later.",
            ctx,
            command,
        )
        .await;
        return;
    };

    if stories.is_empty() {
        text_interaction("There are no stories", ctx, command).await;
        return;
    }

    if let Err(why) = command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message
                        .embed(|embed| embed.title("Action").description(text))
                        .components(|components| {
                            components.create_action_row(|row| {
                                row.create_select_menu(|menu| {
                                    menu.custom_id(DELETE_STORY_MENU).options(|mut options| {
                                        for (story_id, story_name) in stories {
                                            options = options.create_option(|create_option| {
                                                create_option.label(story_name).value(story_id)
                                            });
                                        }
                                        options
                                    })
                                })
                            })
                        })
                        .ephemeral(true)
                })
        })
        .await
    {
        println!("Cannot respond to slash command: {}", why);
    }
}

pub async fn actual_deletion(
    handler: &Handler,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) -> Result<()> {
    let story_id: i64 = message_component
        .data
        .values
        .first()
        .ok_or_else(|| anyhow!("No id selected"))
        .and_then(|id| id.parse::<i64>().map_err(Into::into))?;

    let database = handler.storage.lock().await;
    let story_name = database.delete_story(story_id)?;
    drop(database);

    update_message_text(
        "Deletion",
        format!("Story `{story_name}` successfully deleted"),
        ctx,
        message_component,
    )
    .await?;

    Ok(())
}

pub async fn update_message_text<Ti: ToString, Te: ToString>(
    title: Ti,
    text: Te,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) -> Result<()> {
    message_component
        .create_interaction_response(&ctx.http, |r| {
            r.kind(InteractionResponseType::UpdateMessage)
                .interaction_response_data(|d| {
                    d.embed(|embed| embed.title(title).description(text))
                        .components(|c| c)
                })
        })
        .await?;
    Ok(())
}

async fn fetch_attachment(attachment: &Attachment) -> Result<String, reqwest::Error> {
    println!("Fetching attachment {}", attachment.url);
    // That is not ideal, but somehow there seems to be some issues with certificates and fly.io.
    // Fast fix.
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;

    match client.get(&attachment.url).send().await {
        Ok(response) => response.text().await,
        Err(e) => {
            println!("Error while fetching attachment: {}", e);
            Err(e)
        }
    }
}
