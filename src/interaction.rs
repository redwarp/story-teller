use anyhow::anyhow;
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

use crate::{utils::story_title, Handler};

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
                .interaction_response_data(|message| {
                    message.embed(|embed| embed.title("Action").description(text))
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
            let story_title = story_title(&content);
            if story_title.is_some() {
                let database = handler.database.lock().await;
                let answer = match database.save_story(&content) {
                    Ok(_) => format!(
                        "Successfully uploaded `{}`, adding story `{}`",
                        attachment.filename,
                        story_title.unwrap()
                    ),
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
    let text = "Please select the story you want to delete:";
    let database = handler.database.lock().await;
    let all_stories = database.list_all_stories();

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
) {
    let story_id: Result<u64, _> = message_component
        .data
        .values
        .first()
        .ok_or_else(|| anyhow!("No id selected"))
        .and_then(|id| id.parse::<u64>().map_err(Into::into));
    let story_id = if let Ok(story_id) = story_id {
        story_id
    } else {
        update_message_text("Wrong story selected", ctx, message_component).await;
        return;
    };

    let database = handler.database.lock().await;
    let delete_result = database.delete_story(story_id);
    drop(database);

    if let Ok(story_name) = delete_result {
        update_message_text(
            format!("Story `{story_name}` successfully deleted"),
            ctx,
            message_component,
        )
        .await;
    } else {
        update_message_text("Couldn't delete the story", ctx, message_component).await;
    }
}

async fn update_message_text<T: ToString>(
    text: T,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) {
    if let Err(why) = message_component
        .create_interaction_response(&ctx.http, |r| {
            r.kind(InteractionResponseType::UpdateMessage)
                .interaction_response_data(|d| {
                    d.embed(|embed| embed.title("Action").description(text))
                        .components(|c| c.set_action_rows(vec![]))
                })
        })
        .await
    {
        println!("Cannot respond to slash command: {}", why);
    }
}

async fn fetch_attachment(attachment: &Attachment) -> Result<String, reqwest::Error> {
    match reqwest::get(&attachment.url).await {
        Ok(response) => response.text().await,
        Err(e) => Err(e),
    }
}
