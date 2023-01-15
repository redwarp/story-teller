use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn new<P: AsRef<Path>>(database_path: P) -> Result<Self> {
        let connection = Connection::open(database_path)?;
        let mut check = connection
            .prepare("SELECT name FROM sqlite_schema where type='table' and name='counter'")?;

        let exists = check.exists([])?;

        if !exists {
            println!("No table yet, creating");
            connection
                .execute(
                    "create table if not exists counter(
             id integer primary key,
             count integer not null
         )",
                    [],
                )
                .unwrap();
            connection.execute("insert into counter (id, count) values (0, 0)", [])?;
        }
        drop(check);

        Ok(Self { connection })
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
}
