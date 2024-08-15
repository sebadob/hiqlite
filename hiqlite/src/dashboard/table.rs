use crate::dashboard::handlers::TableFilterRequest;
use crate::network::AppStateExt;
use crate::query::query_map;
use crate::{params, Error, Param, Row};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TableType {
    Table,
    Index,
    View,
    Trigger,
}

impl From<&str> for TableType {
    fn from(value: &str) -> Self {
        match value {
            "table" => Self::Table,
            "index" => Self::Index,
            "view" => Self::View,
            "trigger" => Self::Trigger,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Table {
    typ: TableType,
    name: String,
    tbl_name: String,
    sql: Option<String>,
}

impl<'r> From<crate::Row<'r>> for Table {
    fn from(mut row: Row<'r>) -> Self {
        Self {
            typ: TableType::from(row.get::<String>("type").as_str()),
            name: row.get("name"),
            tbl_name: row.get("tbl_name"),
            sql: row.try_get("sql").ok(),
        }
    }
}

impl Table {
    // pub async fn find(state: &AppStateExt, name: String) -> Result<Self, Error> {
    //     query_map_one(
    //         state,
    //         "SELECT type,name,tbl_name,sql FROM sqlite_master WHERE name = $1",
    //         params!(name),
    //     )
    //     .await
    // }

    pub async fn find_all(state: &AppStateExt) -> Result<Vec<Self>, Error> {
        let res: Vec<Self> = query_map(
            state,
            "SELECT type,name,tbl_name,sql FROM sqlite_master",
            params!(),
        )
        .await?;

        Ok(res)
    }

    pub async fn find_all_filtered(
        state: &AppStateExt,
        filter: TableFilterRequest,
    ) -> Result<Vec<Self>, Error> {
        let res: Vec<Self> = query_map(
            state,
            "SELECT type,name,tbl_name,sql FROM sqlite_master WHERE type = $1",
            params!(filter.as_str()),
        )
        .await?;

        Ok(res)
    }
}

// #[derive(Debug, Serialize)]
// pub struct TableDetails {
//     typ: TableType,
//     name: String,
//     tbl_name: String,
//     sql: Option<String>,
//     columns: Vec<(String, String)>,
// }
//
// impl TableDetails {
//     pub async fn find(state: &AppStateExt, name: String) -> Result<Self, Error> {
//         let sql = format!("SELECT * FROM {}", name);
//         let columns = query_columns(&state.raft_db.read_pool, sql).await?;
//         let t = Table::find(state, name).await?;
//
//         Ok(Self {
//             typ: t.typ,
//             name: t.name,
//             tbl_name: t.tbl_name,
//             sql: t.sql,
//             columns,
//         })
//     }
// }
