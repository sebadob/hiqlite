use rust_embed::{Embed, RustEmbed};
use std::borrow::Cow;

#[derive(Debug)]
pub struct Migration {
    pub id: u32,
    pub name: String,
    /// sha256 hash as hex
    pub hash: String,
    pub content: Vec<u8>,
}

#[macro_export]
macro_rules! embed_migrations {
    ($folder:expr) => {
        pub(crate) mod migrations {
            use crate::migration::Migration;
            use rust_embed::Embed;
            use std::borrow::Cow;

            #[derive(Embed)]
            #[include = "*.sql"]
            #[folder = $folder]
            struct SqlFiles;

            pub(crate) fn build() -> Vec<Migration> {
                let mut files = SqlFiles::iter()
                    .map(|name| {
                        let (id, _) = name.split_once('_').expect(
                            "Migration file names must start with `<integer>_<migration_name>",
                        );
                        let id = id
                            .parse::<u32>()
                            .expect("Migration scripts must start with an increasing integer");
                        (id, name)
                    })
                    .collect::<Vec<(u32, Cow<'static, str>)>>();
                files.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap());

                let mut res: Vec<Migration> = Vec::with_capacity(files.len());

                for (id, file_name) in files {
                    let data = SqlFiles::get(file_name.as_ref()).unwrap();
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
    };
}
