use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::Result;
use rusqlite::Connection;
use twee_v3::Story;
use uuid::Uuid;

use crate::utils::verify_story;

const CREATE_COUNTER: &str = "
create table if not exists counter(
    id integer primary key,
    count integer not null
)";

const CREATE_STORIES: &str = "
create table if not exists stories(
    id integer primary key autoincrement,
    name text not null,
    filename text not null
)";

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

        create_tables(&connection)?;

        if !exists {
            println!("No table yet, creating");
            connection.execute("insert into counter (id, count) values (0, 0)", [])?;
        }
        drop(check);

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
    Ok(())
}
