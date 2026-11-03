<script lang="ts">
    import type { IRow as IRowQuery } from '#lib/types/query_results.js';
    import DataTable from '#lib/components/data_table/DataTable.svelte';
    import { isNumericSqlValue, sqlValueDisplay } from '#lib/utils/sqlValue.js';
    import type { IColumn, IRow } from '#lib/components/data_table/data_table_types.js';

    let { rows = $bindable() }: { rows: IRowQuery[] } = $props();

    let columns: IColumn[] = $state([]);
    let rowsDt: IRow[][] = $state([]);
    $inspect(rowsDt);

    $effect(() => {
        let cols: IColumn[] = [];
        let resRows: IRow[][] = [];

        if (rows.length > 0) {
            for (let i = 0; i < rows[0].columns.length; i++) {
                cols.push({
                    content: rows[0].columns[i].name,
                    initialWidth: '12rem',
                    // TODO this would be improved with iterating over a given set of results and grep
                    // the longest content in each column, but would be pretty resource intense on big tables.
                    // initialWidth: `${8 + col.name.length * .5}rem`,
                    orderType: columnOrderType(rows, i),
                });
            }

            for (let r of rows) {
                let cols = [];
                for (let col of r.columns) {
                    cols.push({
                        content: sqlValueDisplay(col.value),
                    });
                }
                resRows.push(cols);
            }
        }

        columns = cols;
        rowsDt = resRows;
    });

    function columnOrderType(rows: IRowQuery[], index: number): 'number' | 'string' {
        // A leading NULL carries no type information, so the first typed value decides.
        for (let r of rows) {
            let value = r.columns[index]?.value;
            if (value !== undefined && isNumericSqlValue(value)) {
                return 'number';
            }
        }
        return 'string';
    }
</script>

{#if columns.length > 0 && rowsDt.length > 0}
    <DataTable {columns} bind:rows={rowsDt} paginationPageSize={100}>
        <!--{#snippet select(rows: boolean[], close: undefined | (() => void))}-->
        <!--{/snippet}-->
        <!--{#snippet options(row: IRow[], close: undefined | (() => void))}-->
        <!--{/snippet}-->
    </DataTable>
{:else}
    <p>no results</p>
{/if}
