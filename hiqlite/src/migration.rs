use rusqlite::Row;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

pub struct Migrations;

impl Migrations {
    pub fn build<T: RustEmbed>() -> Vec<Migration> {
        let mut files = T::iter()
            .map(|name| {
                let (id, _) = name
                    .split_once('_')
                    .expect("Migration file names must start with `<integer>_<migration_name>");
                let id = id.parse::<u32>().expect(
                    "Migration scripts must start with an increasing integer with \
                    no gaps and starting at index 1",
                );
                (id, name)
            })
            .collect::<Vec<(u32, Cow<'static, str>)>>();

        if files.is_empty() {
            return Vec::default();
        }

        files.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap());
        if let Some((first_id, _)) = files.first() {
            if *first_id != 1 {
                panic!("Migrations must start at index 1");
            }
        }

        let mut res: Vec<Migration> = Vec::with_capacity(files.len());

        for (id, file_name) in files {
            let data = T::get(file_name.as_ref()).unwrap();
            let hash = hex::encode(data.metadata.sha256_hash());
            let content = data.data.to_vec();

            let stripped = file_name
                .strip_suffix(".sql")
                .expect("Migration scripts must always end with .sql");
            let (_, name) = stripped.split_once('_').unwrap();

            let migration = Migration {
                id,
                name: name.to_string(),
                hash,
                content,
            };

            let len = res.len();
            if len > 0 && migration.id != (res[len - 1].id + 1) {
                panic!("");
            }

            res.push(migration);
        }

        res
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub id: u32,
    pub name: String,
    /// sha256 hash as hex
    pub hash: String,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedMigration {
    pub id: u32,
    pub name: String,
    pub ts: i64,
    /// sha256 hash as hex
    pub hash: String,
}

impl<'r> From<&'r Row<'r>> for AppliedMigration {
    fn from(row: &'r Row<'r>) -> Self {
        Self {
            id: row.get_unwrap("id"),
            name: row.get_unwrap("name"),
            ts: row.get_unwrap("ts"),
            hash: row.get_unwrap("hash"),
        }
    }
}
