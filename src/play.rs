use anyhow::anyhow;
use anyhow::Result;
use serenity::{
    model::prelude::interaction::{
        application_command::ApplicationCommandInteraction,
        message_component::MessageComponentInteraction, InteractionResponseType,
    },
    prelude::Context,
};
use twee_v3::Story;

use crate::interaction::update_message_text;
use crate::{interaction::text_interaction, Handler};

pub const START_STORY_MENU: &str = "start_story_menu";

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
    text_interaction("Continuing game", ctx, command).await;
    Ok(())
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
    println!("Let's send the stories");

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

pub async fn actual_start_inner(
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
            "You picked `{story_name}`! Enter `/play` again to start the story",
            story_name = story.title().unwrap()
        ),
        ctx,
        message_component,
    )
    .await;

    Ok(())
}
