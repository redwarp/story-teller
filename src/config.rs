use std::{env, fs::read_to_string, path::Path};

use toml::{value::Map, Value};

pub struct Config {
    content: Value,
}

impl Config {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let content = read_to_string(path).unwrap_or_default();
        let content = content.parse::<Value>().unwrap_or(Value::Table(Map::new()));

        Self { content }
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        env::var(key).ok().or_else(|| {
            self.content
                .get(key)
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
    }
}
