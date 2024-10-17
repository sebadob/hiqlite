use crate::debug;
use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use hiqlite::{params, Client, Error, Param, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
struct Data {
    id: i64,
    id_none: Option<i64>,
    id_opt: Option<i64>,
    name: String,
    name_none: Option<String>,
    name_opt: Option<String>,
    is_bool: bool,
    utc: DateTime<Utc>,
    local: DateTime<Local>,
    offset: DateTime<FixedOffset>,
    naive_date: NaiveDate,
    naive_time: NaiveTime,
    naive_dt: NaiveDateTime,
    json: serde_json::Value,
}

impl<'r> From<hiqlite::Row<'r>> for Data {
    fn from(mut row: Row) -> Self {
        Self {
            id: row.get("id"),
            id_none: row.get("id_none"),
            id_opt: row.get("id_opt"),
            name: row.get("name"),
            name_none: row.get("name_none"),
            name_opt: row.get("name_opt"),
            is_bool: row.get("is_bool"),
            utc: row.get("utc"),
            local: row.get("local"),
            offset: row.get("offset"),
            naive_date: row.get("naive_date"),
            naive_time: row.get("naive_time"),
            naive_dt: row.get("naive_dt"),
            json: row.get("json"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Json {
    id: i64,
    text: String,
}

pub async fn test_type_conversions(client: &Client) -> Result<(), Error> {
    let utc = Utc::now();
    let local = Local::now();
    let offset = utc.fixed_offset();

    let naive_dt = utc.naive_local();
    let naive_date = utc.date_naive();
    let naive_time = naive_dt.time();

    let mut json_map = serde_json::Map::new();
    json_map.insert("id".to_string(), serde_json::Value::from(23));
    json_map.insert(
        "text".to_string(),
        serde_json::Value::from("Some Json Text"),
    );

    let data = Data {
        id: 1,
        id_none: None,
        id_opt: Some(2),
        name: "Name".to_string(),
        name_none: None,
        name_opt: Some("Some Name".to_string()),
        is_bool: true,
        utc,
        local,
        offset,
        naive_date,
        naive_time,
        naive_dt,
        json: serde_json::Value::Object(json_map.clone()),
    };
    debug(&data);

    let d = data.clone();
    let params = params!(
        d.id,
        d.id_none,
        d.id_opt,
        d.name,
        d.name_none,
        d.name_opt,
        d.is_bool,
        d.utc,
        d.local,
        d.offset,
        d.naive_date,
        d.naive_time,
        d.naive_dt,
        d.json
    );
    debug(&params);

    let mut ret: Vec<Result<Data, Error>> = client
        .execute_returning_map(
            r#"INSERT INTO type_conversion
            (id, id_none, id_opt, name, name_none, name_opt, is_bool, utc, local, offset,
            naive_date, naive_time, naive_dt, json)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING *"#,
            params,
        )
        .await?;
    assert_eq!(ret.len(), 1);
    let res: Data = ret.remove(0)?;
    assert_eq!(res, data);

    let slf: Data = client
        .query_map_one(
            "SELECT * FROM type_conversion WHERE id = $1",
            params!(data.id),
        )
        .await?;
    assert_eq!(slf, data);

    // TODO we can't use the automatic conversion with `query_as`, because of the default serialization
    // / deserialization for chrono types is not the one implemented in rusqlite. Can we make this work?
    // let slf: Data = client
    //     .query_as_one(
    //         "SELECT * FROM type_conversion WHERE id = $1",
    //         params!(data.id),
    //     )
    //     .await?;
    // assert_eq!(slf, data);

    Ok(())
}
