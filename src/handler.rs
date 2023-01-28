use anyhow::Result;
use serenity::{
    async_trait,
    model::prelude::{
        command::Command,
        interaction::{message_component::MessageComponentInteraction, Interaction},
        Ready,
    },
    prelude::*,
};

use crate::{
    command::{
        DeleteStoryCommand, PlayCommand, SlashCommand, SlashCommandCreator, StopCommand,
        UploadStoryCommand,
    },
    interaction::{
        actual_deletion, delete_story_interaction, text_interaction, update_message_text,
        upload_story_interaction, DELETE_STORY_MENU,
    },
    persistance::Storage,
    play::{
        actual_start, next_chapter_from_button, next_chapter_from_menu, play_story_interaction,
        stop_story_interaction, the_end, PICK_NEXT_PASSAGE, PICK_NEXT_PASSAGE_BUTTON,
        START_STORY_MENU, THE_END,
    },
};

pub struct Handler {
    pub storage: Mutex<Storage<String>>,
}

impl Handler {
    pub async fn handle_message_component(
        &self,
        ctx: &Context,
        message_component: &MessageComponentInteraction,
    ) -> Result<()> {
        match message_component.data.custom_id.as_str() {
            DELETE_STORY_MENU => actual_deletion(self, ctx, message_component).await?,
            START_STORY_MENU => actual_start(self, ctx, message_component).await?,
            PICK_NEXT_PASSAGE => next_chapter_from_menu(self, ctx, message_component).await?,
            THE_END => the_end(self, ctx, message_component).await?,
            other => {
                if other.starts_with(PICK_NEXT_PASSAGE_BUTTON) {
                    // This is passage with a single selection
                    next_chapter_from_button(self, ctx, message_component).await?;
                }
                println!("Message component {other}");
            }
        }
        Ok(())
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            match command.data.name.as_str() {
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
            if self
                .handle_message_component(&ctx, &message_component)
                .await
                .is_err()
            {
                let _ignored_result = update_message_text(
                    "Error",
                    "Something went wrong, try again later.",
                    &ctx,
                    &message_component,
                )
                .await;
            };
        } else {
            println!("Something happened");
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        Command::set_global_application_commands(&ctx.http, |commands| {
            commands
                .create_slash_command::<UploadStoryCommand>()
                .create_slash_command::<DeleteStoryCommand>()
                .create_slash_command::<PlayCommand>()
                .create_slash_command::<StopCommand>()
        })
        .await
        .unwrap();
    }
}
