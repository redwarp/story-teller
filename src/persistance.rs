use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::Result;
use rusqlite::Connection;
use twee_v3::Story;
use uuid::Uuid;

use crate::play::GameState;
use crate::utils::verify_story;

const CREATE_COUNTER: &str = "
CREATE TABLE IF NOT EXISTS counter(
    id integer primary key,
    count integer not null
);";

const CREATE_STORIES: &str = "
create table if not exists stories(
    id integer primary key autoincrement,
    name text not null,
    filename text not null
);";

const CREATE_STORY_STATE: &str = "
CREATE TABLE IF NOT EXISTS story_state(
    `player_id` TEXT NOT NULL PRIMARY KEY,
    `story_id` INT NOT NULL,
    `current_step` TEXT NOT NULL,
    CONSTRAINT fk_story
        FOREIGN KEY (`story_id`)
        REFERENCES `stories`(`id`)
        ON DELETE CASCADE
);";

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
        let mut check = connection
            .prepare("SELECT name FROM sqlite_schema where type='table' and name='counter'")?;

        let exists = check.exists([])?;
        drop(check);

        create_tables(&connection)?;

        if !exists {
            println!("No table yet, creating");
            connection.execute("insert into counter (id, count) values (0, 0)", [])?;
        }

        Ok(Self {
            connection,
            storage_folder,
        })
    }

    pub fn get_count(&self) -> Result<u32> {
        let mut stmt = self
            .connection
            .prepare("select count from counter where id = 0")?;
        let count = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    pub fn increment_count(&self) -> Result<()> {
        let mut stmt = self
            .connection
            .prepare("update counter set count = count + 1 where id = 0")?;
        stmt.execute([])?;
        Ok(())
    }

    pub fn save_story(&self, story_content: &str) -> Result<()> {
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

        fs::write(&file_path, story_content)?;
        if let Err(e) = self.connection.execute(
            "INSERT INTO stories (name, filename) VALUES (?1, ?2)",
            (name, filename.as_str()),
        ) {
            println!("Couldn't save story to database, deleting file");
            fs::remove_file(file_path)?;

            return Err(e.into());
        }

        Ok(())
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

    pub fn list_all_stories(&self) -> Result<Vec<(i64, String)>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name FROM stories")
            .unwrap();
        let stories = statement
            .query_map([], |row| {
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
            "INSERT OR REPLACE into story_state (player_id, story_id, current_step) VALUES
        (?1, ?2, ?3)";
        self.connection.execute(
            QUERY,
            (
                &game_state.player_id,
                &game_state.story_id,
                &game_state.current_chapter,
            ),
        )?;
        Ok(())
    }

    pub fn retrieve_game_state(&self, player_id: &str) -> Result<GameState> {
        const QUERY: &str = "SELECT story_id, current_step FROM story_state WHERE player_id = ?";

        let (story_id, current_step) = self.connection.query_row(QUERY, [player_id], |row| {
            let story_id: i64 = row.get(0)?;
            let current_step: String = row.get(1)?;
            Ok((story_id, current_step))
        })?;

        Ok(GameState::new(
            player_id.to_string(),
            story_id,
            current_step,
        ))
    }

    pub fn clear_game_state(&self, player_id: &str) -> Result<()> {
        const QUERY: &str = "DELETE FROM story_state WHERE player_id = ?";

        self.connection.execute(QUERY, [player_id])?;

        Ok(())
    }

    pub fn load_story_content(&self, story_id: i64) -> Result<String> {
        const QUERY: &str = "SELECT filename FROM stories WHERE id = ?";
        let filename: String = self
            .connection
            .query_row(QUERY, [story_id], |row| row.get(0))?;

        let path = self.stories_folder()?.join(filename);
        let content = fs::read_to_string(path)?;

        Ok(content)
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
    connection.execute(CREATE_COUNTER, [])?;
    connection.execute(CREATE_STORIES, [])?;
    connection.execute(CREATE_STORY_STATE, [])?;
    Ok(())
}
