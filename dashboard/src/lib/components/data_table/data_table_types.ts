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
    // will be used as the link if `IColumn.showAs` is set to `a` or `a_blank`
    href?: string,
    onClick?: (ev: Event, row: number) => void,
}

export interface IDataTable {
    caption?: string,
    columns: IColumn[],
    showColumns?: boolean[],
    rows: IRow[][],
    // gridTemplateColumns: string,
    options?: Snippet<[row: IRow[], close: undefined | (() => void)]>,
    offsetLeftOptions?: string;
    offsetTopOptions?: string;
    offsetLeftColumnSelect?: string;
    paginationCompact?: boolean,
    select?: Snippet<[rows: boolean[], close: undefined | (() => void)]>,
    selectedRows?: boolean[],
    width?: string,
    maxWidth?: string,
    minWidthColPx?: number,
}