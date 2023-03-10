use anyhow::{anyhow, Result};
use serenity::{
    builder::CreateComponents,
    model::prelude::interaction::{
        application_command::ApplicationCommandInteraction,
        message_component::MessageComponentInteraction, InteractionResponseType,
    },
    prelude::Context,
};
use twee_v3::Passage;

use crate::{
    interaction::{text_interaction, update_message_text},
    Handler,
};

pub const START_STORY_MENU: &str = "start_story_menu";
pub const PICK_NEXT_PASSAGE: &str = "pick_next_passage";
pub const PICK_NEXT_PASSAGE_BUTTON: &str = "pick_next_passage_button";
pub const THE_END: &str = "the_end";

pub struct GameState {
    pub player_id: String,
    pub guild_id: String,
    pub story_id: i64,
    pub current_chapter: String,
}

impl GameState {
    pub fn new(
        player_id: String,
        guild_id: String,
        story_id: i64,
        current_chapter: String,
    ) -> Self {
        Self {
            player_id,
            guild_id,
            story_id,
            current_chapter,
        }
    }
}

pub async fn stop_story_interaction(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) {
    if stop_story_interaction_inner(handler, ctx, command)
        .await
        .is_err()
    {
        println!("Error!");
        text_interaction("Error while playing the story", ctx, command).await;
    }
}

async fn stop_story_interaction_inner(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<()> {
    let storage = handler.storage.lock().await;
    let player_id = command.user.id.to_string();
    let guild_id = command
        .guild_id
        .ok_or_else(|| anyhow!("No guild id"))?
        .to_string();
    storage.clear_game_state(&player_id, &guild_id)?;
    drop(storage);

    text_interaction(
        "Current story stopped, start again with the `/play` command",
        ctx,
        command,
    )
    .await;

    Ok(())
}

pub async fn play_story_interaction(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) {
    if play_story_interaction_inner(handler, ctx, command)
        .await
        .is_err()
    {
        println!("Error!");
        text_interaction("Error while playing the story", ctx, command).await;
    }
}

async fn play_story_interaction_inner(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<()> {
    let database = handler.storage.lock().await;
    let player_id = command.user.id.to_string();
    let guild_id = command
        .guild_id
        .ok_or_else(|| anyhow!("No guild id"))?
        .to_string();

    let game_state_result = database.retrieve_game_state(&player_id, &guild_id);
    drop(database);

    match game_state_result {
        Ok(game_state) => continue_game(&game_state, handler, ctx, command).await?,
        Err(_) => start_new_game(handler, ctx, command).await?,
    }

    Ok(())
}

async fn continue_game(
    game_state: &GameState,
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<()> {
    println!("Continuing game");

    let mut database = handler.storage.lock().await;
    let story = database.get_story(game_state.story_id)?;
    drop(database);

    let passage = story
        .get_passage(&game_state.current_chapter)
        .ok_or_else(|| anyhow!("Couldn't retrieve passage"))?;

    let mut passage_content = String::new();
    for node in passage.nodes() {
        match node {
            twee_v3::ContentNode::Text(text) => passage_content.push_str(text),
            twee_v3::ContentNode::Link { text, target: _ } => {
                passage_content.push_str(&format!("`{text}`"))
            }
        };
    }

    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message
                        .embed(|embed| embed.title(passage.title()).description(passage_content))
                        .components(|components| add_story_components(components, &passage))
                        .ephemeral(true)
                })
        })
        .await?;

    Ok(())
}

async fn start_new_game(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<()> {
    let guild_id = command
        .guild_id
        .ok_or_else(|| anyhow!("No guild id"))?
        .to_string();

    println!("Starting new game");
    let storage = handler.storage.lock().await;
    let stories = storage.list_guild_stories(&guild_id)?;

    if stories.is_empty() {
        println!("There are no stories");
        text_interaction("There are no stories", ctx, command).await;
        println!("Returning");
        return Ok(());
    }
    let text = "Please select a story to start playing";

    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message
                        .embed(|embed| embed.title("Let's go").description(text))
                        .components(|components| {
                            components.create_action_row(|row| {
                                row.create_select_menu(|menu| {
                                    menu.custom_id(START_STORY_MENU).options(|mut options| {
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
        .await?;

    Ok(())
}

pub async fn actual_start(
    handler: &Handler,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) -> Result<()> {
    let story_id = message_component
        .data
        .values
        .first()
        .ok_or_else(|| anyhow!("No id selected"))
        .and_then(|id| id.parse::<i64>().map_err(Into::into))?;

    let guild_id = message_component
        .guild_id
        .ok_or_else(|| anyhow!("No guild id"))?
        .to_string();

    let mut storage = handler.storage.lock().await;
    let story = storage.get_story(story_id)?;
    drop(storage);

    let start = story
        .start()
        .ok_or_else(|| anyhow!("Story without start"))?;
    let player_id = message_component.user.id.to_string();
    let game_state = GameState::new(player_id, guild_id, story_id, start.title().to_string());
    {
        let storage = handler.storage.lock().await;
        storage.update_game_state(&game_state)?;
    }

    update_message_text(
        "Let's go",
        format!(
            "Your story `{story_name}` is starting!",
            story_name = story.title().unwrap()
        ),
        ctx,
        message_component,
    )
    .await?;

    let passage = story
        .get_passage(&game_state.current_chapter)
        .ok_or_else(|| anyhow!("Couldn't retrieve passage"))?;

    let mut passage_content = String::new();
    for node in passage.nodes() {
        match node {
            twee_v3::ContentNode::Text(text) => passage_content.push_str(text),
            twee_v3::ContentNode::Link { text, target: _ } => {
                passage_content.push_str(&format!("`{text}`"))
            }
        };
    }

    message_component
        .create_followup_message(&ctx.http, |message| {
            message
                .embed(|embed| embed.title(passage.title()).description(passage_content))
                .components(|components| add_story_components(components, &passage))
                .ephemeral(true)
        })
        .await?;

    Ok(())
}

pub async fn next_chapter_from_menu(
    handler: &Handler,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) -> Result<()> {
    let chapter_name = message_component
        .data
        .values
        .first()
        .ok_or_else(|| anyhow!("No chapter selected"))?;

    next_chapter(handler, ctx, message_component, chapter_name).await
}

pub async fn next_chapter_from_button(
    handler: &Handler,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) -> Result<()> {
    let chapter_name = &message_component.data.custom_id[PICK_NEXT_PASSAGE_BUTTON.len()..];

    next_chapter(handler, ctx, message_component, chapter_name).await
}

pub async fn next_chapter(
    handler: &Handler,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
    chapter_name: &str,
) -> Result<()> {
    let mut database = handler.storage.lock().await;
    let player_id = message_component.user.id.to_string();
    let guild_id = message_component
        .guild_id
        .ok_or_else(|| anyhow!("No guild id"))?
        .to_string();

    let game_state = database.retrieve_game_state(&player_id, &guild_id)?;
    let story = database.get_story(game_state.story_id)?;
    drop(database);

    // Update the previous interaction to remove the menu.
    message_component.defer(&ctx.http).await?;
    message_component
        .edit_original_interaction_response(&ctx.http, |response| response.components(|c| c))
        .await?;

    let passage = story
        .get_passage(chapter_name)
        .ok_or_else(|| anyhow!("Couldn't retrieve passage"))?;

    let mut passage_content = String::new();
    for node in passage.nodes() {
        match node {
            twee_v3::ContentNode::Text(text) => passage_content.push_str(text),
            twee_v3::ContentNode::Link { text, target: _ } => {
                passage_content.push_str(&format!("`{text}`"))
            }
        };
    }

    message_component
        .create_followup_message(&ctx.http, |followup| {
            followup
                .allowed_mentions(|mentions| mentions.replied_user(true))
                .embed(|embed| embed.title(passage.title()).description(passage_content))
                .components(|components| add_story_components(components, &passage))
                .ephemeral(true)
        })
        .await?;

    let database = handler.storage.lock().await;

    if passage.links().count() > 0 {
        database.update_game_state(&GameState {
            current_chapter: chapter_name.to_string(),
            ..game_state
        })?;
    } else {
        database.clear_game_state(&player_id, &guild_id)?;
    }

    Ok(())
}

pub async fn the_end(
    handler: &Handler,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) -> Result<()> {
    let player_id = message_component.user.id.to_string();
    let guild_id = message_component
        .guild_id
        .ok_or_else(|| anyhow!("No guild id"))?
        .to_string();

    {
        let database = handler.storage.lock().await;
        database.clear_game_state(&player_id, &guild_id)?;
    }

    message_component.defer(&ctx.http).await?;
    message_component
        .edit_original_interaction_response(&ctx.http, |response| response.components(|c| c))
        .await?;

    message_component
        .create_followup_message(&ctx.http, |followup| {
            followup
                .allowed_mentions(|mentions| mentions.replied_user(true))
                .embed(|embed| {
                    embed.title("The end").description(
                        "That's it for now! To start a new session, use the `/play` command.",
                    )
                })
                .ephemeral(true)
        })
        .await?;

    Ok(())
}

fn add_story_components<'a, 'b>(
    components: &'a mut CreateComponents,
    passage: &'b Passage<&'b str>,
) -> &'a mut CreateComponents {
    match passage.links().count() {
        0 => components.create_action_row(|row| {
            row.create_button(|create_button| create_button.custom_id(THE_END).label("The end"))
        }),
        1 => components.create_action_row(|row| {
            let link = passage.links().next().expect("one link");
            row.create_button(|create_button| {
                create_button
                    .custom_id(format!("{}{}", PICK_NEXT_PASSAGE_BUTTON, link.target))
                    .label(link.text)
            })
        }),
        _ => components.create_action_row(|row| {
            row.create_select_menu(|menu| {
                menu.custom_id(PICK_NEXT_PASSAGE)
                    .placeholder("Next chapter")
                    .options(|mut options| {
                        for node in passage.links() {
                            options = options.create_option(|create_option| {
                                create_option.label(node.text).value(node.target)
                            });
                        }
                        options
                    })
            })
        }),
    }
}
