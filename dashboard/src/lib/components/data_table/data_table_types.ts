import type {Snippet} from "svelte";

export interface IColumn {
    content: string,
    // accepts all valid css values for `grid-template-columns`
    initialWidth: string,
    // change the way the data inside the `<td>` is rendered
    showAs?: 'a' | 'a_blank' | 'copyToClip' | 'check',
    orderType?: 'string' | 'number',
}

export interface IRow {
    content: string | number | boolean,
    muted?: boolean,
    // will be used as the link if `IColumn.showAs` is set to `a` or `a_blank`
    href?: string,
    onClick?: (ev: Event, row: number) => void,
    withIcon?: 'file' | 'folder',
}

export interface IDataTable {
    caption?: Snippet,
    columns: IColumn[],
    showColumns?: boolean[],
    rows: IRow[][],
    // gridTemplateColumns: string,
    options?: Snippet<[row: IRow[], close: undefined | (() => void)]>,
    highlight?: number;
    offsetLeftOptions?: string;
    offsetTopOptions?: string;
    paginationCompact?: boolean,
    paginationDisabled?: boolean,
    /// any of: 5, 7, 10, 15, 20, 30, 50, 100
    paginationPageSize?: number,
    select?: Snippet<[rows: boolean[], close: undefined | (() => void)]>,
    selectInitHide?: boolean,
    selectedRows?: boolean[],
    width?: string,
    maxWidth?: string,
    minWidthColPx?: number,
}