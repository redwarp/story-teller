use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use rusqlite::Connection;
use twee_v3::Story;
use uuid::Uuid;

use crate::{play::GameState, utils::verify_story};

const CREATE_STORIES: &str = "
create table if not exists stories(
    id integer PRIMARY KEY AUTOINCREMENT,
    guild_id TEXT NOT NULL,
    name text not null,
    filename text not null
);";

const CREATE_STORY_STATE: &str = "
CREATE TABLE IF NOT EXISTS story_state(
    `player_id` TEXT NOT NULL,
    `guild_id` TEXT NOT NULL,
    `story_id` INT NOT NULL,
    `current_step` TEXT NOT NULL,
    PRIMARY KEY(`player_id`, `guild_id`),
    CONSTRAINT fk_story
        FOREIGN KEY (`story_id`)
        REFERENCES `stories`(`id`)
        ON DELETE CASCADE
);";

pub enum SaveStory {
    New,
    Update,
}

pub struct Storage<P: AsRef<Path>> {
    storage_folder: P,
    connection: Connection,
}

impl<P> Storage<P>
where
    P: AsRef<Path>,
{
    pub fn new(storage_folder: P) -> Result<Self> {
        if !storage_folder.as_ref().exists() {
            fs::create_dir_all(&storage_folder)?;
        }
        let database_path = storage_folder.as_ref().join("data.sqlite");
        let connection = Connection::open(database_path)?;

        create_tables(&connection)?;

        Ok(Self {
            connection,
            storage_folder,
        })
    }

    pub fn save_story(&self, guild_id: &str, story_content: &str) -> Result<SaveStory> {
        if !verify_story(story_content) {
            return Err(anyhow!("Invalid story"));
        }

        let story = Story::try_from(story_content).expect("Already verified");

        let name = if let Some(title) = story.title() {
            title
        } else {
            return Err(anyhow!("Story without title"));
        };

        let (filename, file_path) = loop {
            let filename = format!("{}.twee", Uuid::new_v4());
            let file_path = self.stories_folder()?.join(&filename);
            if !file_path.exists() {
                break (filename, file_path);
            }
        };

        let did_overwrite = self.cleanup_previous(guild_id, name)?;

        fs::write(&file_path, story_content)?;
        if let Err(e) = self.connection.execute(
            "INSERT INTO stories (guild_id, name, filename) VALUES (?1, ?2, ?3)",
            (guild_id, name, filename.as_str()),
        ) {
            println!("Couldn't save story to database, deleting file");
            fs::remove_file(file_path)?;

            return Err(e.into());
        }

        Ok(match did_overwrite {
            true => SaveStory::Update,
            false => SaveStory::New,
        })
    }

    fn cleanup_previous(&self, guild_id: &str, name: &str) -> Result<bool> {
        const QUERY: &str = "SELECT id FROM stories WHERE guild_id = ?1 AND name = ?2";
        match self
            .connection
            .query_row(QUERY, [guild_id, name], |row| row.get::<_, i64>(0))
        {
            Ok(story_id) => {
                self.delete_story(story_id)?;
                Ok(true)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete story with the id, and returns the name of the deleted story.
    pub fn delete_story(&self, story_id: i64) -> Result<String> {
        let (name, filename) = self.connection.query_row(
            "SELECT name, filename FROM stories WHERE `id`=?",
            [story_id],
            |row| {
                let name: String = row.get(0)?;
                let filename: String = row.get(1)?;
                Ok((name, filename))
            },
        )?;

        let count = self
            .connection
            .execute("DELETE FROM stories WHERE `id` = ?1", [story_id])?;

        if count > 0 {
            // Deleting the story file, we don't care that much if it fails.
            if let Ok(story_folder) = self.stories_folder() {
                let file_path = story_folder.join(filename);
                let _ = fs::remove_file(file_path);
            }

            Ok(name)
        } else {
            Err(anyhow!("No stories was deleted"))
        }
    }

    pub fn list_guild_stories(&self, guild_id: &str) -> Result<Vec<(i64, String)>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name FROM stories WHERE guild_id = ?1")
            .unwrap();
        let stories = statement
            .query_map([guild_id], |row| {
                let id: i64 = row.get(0)?;
                let name: String = row.get(1)?;
                Ok((id, name))
            })?
            .collect::<Result<Vec<_>, _>>();
        let stories = stories?;
        Ok(stories)
    }

    pub fn update_game_state(&self, game_state: &GameState) -> Result<()> {
        const QUERY: &str =
            "INSERT OR REPLACE into story_state (player_id, guild_id, story_id, current_step) VALUES
        (?1, ?2, ?3, ?4)";
        self.connection.execute(
            QUERY,
            (
                &game_state.player_id,
                &game_state.guild_id,
                &game_state.story_id,
                &game_state.current_chapter,
            ),
        )?;
        Ok(())
    }

    pub fn retrieve_game_state(&self, player_id: &str, guild_id: &str) -> Result<GameState> {
        const QUERY: &str =
            "SELECT story_id, current_step FROM story_state WHERE player_id = ?1 AND guild_id = ?2";

        let (story_id, current_step) =
            self.connection
                .query_row(QUERY, [player_id, guild_id], |row| {
                    let story_id: i64 = row.get(0)?;
                    let current_step: String = row.get(1)?;
                    Ok((story_id, current_step))
                })?;

        Ok(GameState::new(
            player_id.to_string(),
            guild_id.to_string(),
            story_id,
            current_step,
        ))
    }

    pub fn clear_game_state(&self, player_id: &str, guild_id: &str) -> Result<()> {
        const QUERY: &str = "DELETE FROM story_state WHERE player_id = ?1 AND guild_id = ?2";

        self.connection.execute(QUERY, [player_id, guild_id])?;

        Ok(())
    }

    pub fn load_story(&self, story_id: i64) -> Result<Story<String>> {
        const QUERY: &str = "SELECT filename FROM stories WHERE id = ?";
        let filename: String = self
            .connection
            .query_row(QUERY, [story_id], |row| row.get(0))?;

        let path = self.stories_folder()?.join(filename);
        let content = fs::read_to_string(path)?;
        let story = Story::try_from(content)?;

        Ok(story)
    }

    fn stories_folder(&self) -> Result<PathBuf> {
        let folder = self.storage_folder.as_ref().join("stories");
        if !folder.exists() {
            fs::create_dir_all(&folder)?;
        }
        Ok(folder)
    }
}

fn create_tables(connection: &Connection) -> Result<()> {
    connection.execute(CREATE_STORIES, [])?;
    connection.execute(CREATE_STORY_STATE, [])?;
    Ok(())
}
