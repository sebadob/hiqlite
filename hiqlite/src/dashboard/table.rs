use crate::dashboard::handlers::TableFilterRequest;
use crate::network::AppStateExt;
use crate::query::query_map;
use crate::{Error, Param, Params};
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

impl From<&mut crate::Row<'_>> for Table {
    fn from(row: &mut crate::Row<'_>) -> Self {
        Self {
            typ: TableType::from(row.get::<String>("type").as_str()),
            name: row.get("name"),
            tbl_name: row.get("tbl_name"),
            sql: row.try_get("sql").ok(),
        }
    }
}

impl Table {
    pub async fn find_all(state: &AppStateExt) -> Result<Vec<Self>, Error> {
        let res: Vec<Self> = query_map(
            state,
            "SELECT type,name,tbl_name,sql FROM sqlite_master",
            Params::new(),
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
            vec![Param::Text(filter.as_str().to_string())],
        )
        .await?;

        Ok(res)
    }
}
