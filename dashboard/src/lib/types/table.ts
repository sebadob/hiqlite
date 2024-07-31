export interface ITable {
    typ: string,
    name: string,
    tbl_name: string,
    sql: string,
}

export enum ITableView {
    Table = 'table',
    Index = 'index',
    Trigger = 'view',
    View = 'trigger',
}