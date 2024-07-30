export interface IRow {
    columns: IColumn[],
}

export interface IColumn {
    name: string,
    value: SqlValue,
}

export type SqlValue = { "Null": null }
    | { "Integer": number }
    | { "Real": number }
    | { "Text": string }
    | { "Blob": ArrayBuffer };
