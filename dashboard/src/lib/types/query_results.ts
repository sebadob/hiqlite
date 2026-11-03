export interface IRow {
    columns: IColumn[],
}

export interface IColumn {
    name: string,
    value: SqlValue,
}

/// Mirrors `ValueOwned` in the Rust crate: serde serializes unit variants as a plain
/// string, so a SQL NULL arrives as "Null", not as an object.
export type SqlValue = 'Null'
    | { 'Integer': number }
    | { 'Real': number }
    | { 'Text': string }
    | { 'Blob': number[] };
