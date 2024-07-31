<script lang="ts">
    import type {IRow, SqlValue} from "$lib/types/query_results";

    let {rows = $bindable()}: { rows: IRow[] } = $props();

    function buf2hex(buffer: ArrayBuffer) {
        return [...new Uint8Array(buffer)]
            .map(x => x.toString(16).padStart(2, '0'))
            .join('');
    }

    function sqlToStr(value: SqlValue) {
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
    {#if rows.length > 0}
        <div class="row head">
            {#each rows[0].columns as column (column.name)}
                <div class="col">
                    <b>{column.name}</b>
                </div>
            {/each}
        </div>

        <div class="rows">
            {#each rows as row}
                <div class="row">
                    {#each row.columns as column}
                        <div class="col">
                            {sqlToStr(column.value)}
                        </div>
                    {/each}
                </div>
            {/each}
        </div>
    {/if}
</div>

<style>
    #query-results {
        width: 100%;
        height: 100%;
        overflow-x: auto;
    }

    .row {
        display: flex;
    }

    .row:nth-child(even) {
        background: rgba(0, 0, 0, .1);
    }

    .head {
        background: rgba(0, 0, 0, .15);
    }

    .rows {
        overflow-y: auto;
    }

    .col {
        width: 10rem;
        padding: .2rem;
        word-wrap: break-word;
        border-right: 1px solid rgba(0, 0, 0, .1);
    }
</style>
