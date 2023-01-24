use std::time::Duration;

use anyhow::anyhow;
use anyhow::Result;
use serenity::builder::CreateComponents;
use serenity::{
    model::prelude::interaction::{
        application_command::ApplicationCommandInteraction,
        message_component::MessageComponentInteraction, InteractionResponseType,
    },
    prelude::Context,
};
use twee_v3::Passage;
use twee_v3::Story;

use crate::interaction::update_message_text;
use crate::{interaction::text_interaction, Handler};

pub const START_STORY_MENU: &str = "start_story_menu";
pub const PICK_NEXT_PASSAGE: &str = "pick_next_passage";

pub struct GameState {
    pub player_id: String,
    pub story_id: i64,
    pub current_chapter: String,
}

impl GameState {
    pub fn new(player_id: String, story_id: i64, current_chapter: String) -> Self {
        Self {
            player_id,
            story_id,
            current_chapter,
        }
    }
}

pub async fn play_story_interaction(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) {
    if let Err(_) = play_story_interaction_inner(handler, ctx, command).await {
        println!("Error!");
        text_interaction("Error while playing the story", ctx, command).await;
    }
}

async fn play_story_interaction_inner(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<()> {
    println!("Playing!");
    let database = handler.storage.lock().await;
    let player_id = command.user.id.to_string();

    let game_state_result = database.retrieve_game_state(&player_id);
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

    let database = handler.storage.lock().await;
    let story_content = database.load_story_content(game_state.story_id)?;
    drop(database);
    let story = Story::try_from(story_content.as_str()).map_err(|_| anyhow!("Parsing error"))?;

    let passage = story
        .get_passage(&game_state.current_chapter)
        .ok_or(anyhow!("Couldn't retrieve passage"))?;

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
                        .components(|components| add_story_components(components, passage))
                })
        })
        .await?;

    Ok(())
}

fn add_story_components<'a, 'b>(
    components: &'a mut CreateComponents,
    passage: &'b Passage<'b>,
) -> &'a mut CreateComponents {
    if passage.links().count() > 0 {
        components.create_action_row(|row| {
            row.create_select_menu(|menu| {
                menu.custom_id(PICK_NEXT_PASSAGE).options(|mut options| {
                    for node in passage.links() {
                        options = options.create_option(|create_option| {
                            create_option.label(node.text).value(node.target)
                        });
                    }
                    options
                })
            })
        })
    } else {
        components
    }
}

async fn start_new_game(
    handler: &Handler,
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<()> {
    println!("Starting new game");
    let storage = handler.storage.lock().await;
    let stories = storage.list_all_stories()?;

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
                        .embed(|embed| embed.title("Action").description(text))
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
                        .title("Let's start the game!")
                })
        })
        .await?;

    Ok(())
}

pub async fn actual_start(
    handler: &Handler,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) {
    if let Err(_) = actual_start_inner(handler, ctx, message_component).await {
        update_message_text("Couldn't start the story", ctx, message_component).await;
    }
}

async fn actual_start_inner(
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

    let storage = handler.storage.lock().await;
    let content = storage.load_story_content(story_id)?;
    drop(storage);

    let story = Story::try_from(content.as_str()).map_err(|_| anyhow!("Parsing error"))?;
    let start = story
        .start()
        .ok_or_else(|| anyhow!("Story without start"))?;
    let player_id = message_component.user.id.to_string();
    {
        let storage = handler.storage.lock().await;
        storage.update_game_state(GameState::new(
            player_id,
            story_id,
            start.title().to_string(),
        ))?;
    }

    update_message_text(
        format!(
            "Your story `{story_name}` is starting!",
            story_name = story.title().unwrap()
        ),
        ctx,
        message_component,
    )
    .await;

    tokio::time::sleep(Duration::from_millis(2000)).await;

    let follow = message_component
        .create_followup_message(&ctx.http, |follow_up| {
            follow_up.embed(|embed| embed.title("Action").description("Let's gooo!"))
        })
        .await;
    if let Err(why) = follow {
        println!("Cannot send follow up: {}", why);
    }

    Ok(())
}

pub async fn actual_passage(
    handler: &Handler,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) {
    if let Err(_) = actual_passage_inner(handler, ctx, message_component).await {
        update_message_text("Couldn't start the story", ctx, message_component).await;
    }
}

async fn actual_passage_inner(
    handler: &Handler,
    ctx: &Context,
    message_component: &MessageComponentInteraction,
) -> Result<()> {
    let next_chapter = message_component
        .data
        .values
        .first()
        .ok_or_else(|| anyhow!("No chapter selected"))?;

    let database = handler.storage.lock().await;
    let player_id = message_component.user.id.to_string();

    let game_state = database.retrieve_game_state(&player_id)?;
    let story_content = database.load_story_content(game_state.story_id)?;
    drop(database);

    let story = Story::try_from(story_content.as_str()).map_err(|_| anyhow!("Parsing error"))?;

    let current_passage = story
        .get_passage(&game_state.current_chapter)
        .ok_or(anyhow!("Couldn't retrieve passage"))?;

    let mut current_passage_content = String::new();
    for node in current_passage.nodes() {
        match node {
            twee_v3::ContentNode::Text(text) => current_passage_content.push_str(text),
            twee_v3::ContentNode::Link { text, target: _ } => {
                current_passage_content.push_str(&format!("`{text}`"))
            }
        };
    }

    message_component
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message
                        .embed(|embed| {
                            embed
                                .title(current_passage.title())
                                .description(current_passage_content)
                        })
                        .components(|components| components.set_action_rows(vec![]))
                })
        })
        .await?;

    let passage = story
        .get_passage(&next_chapter)
        .ok_or(anyhow!("Couldn't retrieve passage"))?;

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
                .components(|components| add_story_components(components, passage))
        })
        .await?;

    let database = handler.storage.lock().await;

    if passage.links().count() > 0 {
        database.update_game_state(GameState {
            current_chapter: next_chapter.clone(),
            ..game_state
        })?;
    } else {
        database.clear_game_state(&player_id)?;
    }

    Ok(())
}
