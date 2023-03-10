use serenity::{
    builder::{CreateApplicationCommand, CreateApplicationCommands},
    model::{prelude::command::CommandOptionType, Permissions},
};

pub trait SlashCommand {
    const NAME: &'static str;
    fn create_application_command(
        command: &mut CreateApplicationCommand,
    ) -> &mut CreateApplicationCommand;
}

pub struct UploadStoryCommand;

impl SlashCommand for UploadStoryCommand {
    const NAME: &'static str = "uploadstory";

    fn create_application_command(
        command: &mut CreateApplicationCommand,
    ) -> &mut CreateApplicationCommand {
        command
            .name(Self::NAME)
            .description("Upload a story")
            .default_member_permissions(Permissions::ADMINISTRATOR)
            .create_option(|option| {
                option
                    .kind(CommandOptionType::Attachment)
                    .name("file")
                    .required(true)
                    .description("The story to upload")
            })
    }
}

pub struct DeleteStoryCommand;

impl SlashCommand for DeleteStoryCommand {
    const NAME: &'static str = "deletestory";

    fn create_application_command(
        command: &mut CreateApplicationCommand,
    ) -> &mut CreateApplicationCommand {
        command
            .name(Self::NAME)
            .description("Delete a story hosted on the guild")
            .default_member_permissions(Permissions::ADMINISTRATOR)
    }
}

pub struct PlayCommand;

impl SlashCommand for PlayCommand {
    const NAME: &'static str = "play";

    fn create_application_command(
        command: &mut CreateApplicationCommand,
    ) -> &mut CreateApplicationCommand {
        command
            .name(Self::NAME)
            .description("Play an interactive story")
    }
}

pub struct StopCommand;

impl SlashCommand for StopCommand {
    const NAME: &'static str = "stop";

    fn create_application_command(
        command: &mut CreateApplicationCommand,
    ) -> &mut CreateApplicationCommand {
        command
            .name(Self::NAME)
            .description("Stop your current interactive story")
    }
}

pub trait SlashCommandCreator {
    fn create_slash_command<S: SlashCommand>(&mut self) -> &mut Self;
}

impl SlashCommandCreator for CreateApplicationCommands {
    fn create_slash_command<S: SlashCommand>(&mut self) -> &mut Self {
        self.create_application_command(|command| {
            S::create_application_command(command).dm_permission(false)
        })
    }
}
