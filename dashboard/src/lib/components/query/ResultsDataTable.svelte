<script lang="ts">
    import type {IRow as IRowQuery, SqlValue} from "$lib/types/query_results";
    import DataTable from "$lib/components/data_table/DataTable.svelte";
    import type {IColumn, IRow} from "$lib/components/data_table/data_table_types";

    let {rows = $bindable()}: { rows: IRowQuery[] } = $props();

    let columns: IColumn[] = $state([]);
    let rowsDt: IRow[][] = $state([]);
    $inspect(rowsDt);

    $effect(() => {
        let cols: IColumn[] = [];
        let resRows: IRow[][] = [];

        if (rows.length > 0) {
            for (let col of rows[0].columns) {
                cols.push({
                    content: col.name,
                    initialWidth: '12rem',
                    // initialWidth: `${8 + col.name.length * .5}rem`,
                    orderType: columnOrderType(col.value),
                })
            }

            for (let r of rows) {
                let cols = [];
                for (let col of r.columns) {
                    cols.push({
                        content: sqlInnerValue(col.value),
                    })
                }
                resRows.push(cols);
            }
        }

        columns = cols;
        rowsDt = resRows;
    });

    function buf2hex(buffer: ArrayBuffer) {
        return [...new Uint8Array(buffer)]
            .map(x => x.toString(16).padStart(2, '0'))
            .join('');
    }

    function columnOrderType(value: SqlValue) {
        if (value.hasOwnProperty('Integer') || value.hasOwnProperty('Real')) {
            return 'number';
        }
        return 'string';
    }

    function sqlInnerValue(value: SqlValue) {
        if (value.hasOwnProperty('Integer')) {
            return value.Integer;
        } else if (value.hasOwnProperty('Real')) {
            return value.Real;
        } else if (value.hasOwnProperty('Text')) {
            return value.Text;
        } else if (value.hasOwnProperty('Blob')) {
            return `x'${buf2hex(value.Blob)}'`;
        }

        return 'NULL';
    }
</script>

<div id="query-results">
    {#if columns.length > 0 && rowsDt.length > 0}
        <DataTable {columns} bind:rows={rowsDt}>
            <!--{#snippet select(rows: boolean[], close: undefined | (() => void))}-->
            <!--{/snippet}-->
            <!--{#snippet options(row: IRow[], close: undefined | (() => void))}-->
            <!--{/snippet}-->
        </DataTable>
    {:else}
        <p>no results</p>
    {/if}
</div>

<style>
    #query-results {
        display: block;
        /*max-width: 40rem;*/
        flex: 1;
        /*max-width: var(--width-inner);*/
        overflow: scroll;
        /*display: flex;*/
    }
</style>