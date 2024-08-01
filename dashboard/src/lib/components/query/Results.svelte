<script lang="ts">
    import type {IColumn, IRow, SqlValue} from "$lib/types/query_results";

    let {rows = $bindable()}: { rows: IRow[] } = $props();

    let cols: IColumn[][] = $state([[]]);

    $effect(() => {
        if (rows.length === 0) {
            cols = [[]];
            return;
        }

        let newCols: IColumn[][] = [];
        for (let i = 0; i < rows[0].columns.length; i++) {
            newCols.push([]);
        }

        for (let row of rows) {
            let columns = row.columns;
            for (let i = 0; i < columns.length; i++) {
                newCols[i].push(columns[i]);
            }
        }

        cols = newCols;
    })

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
    {#each cols as col}
        <div class="col">
            <div class="head">
                <b>{col[0]?.name}</b>
            </div>

            {#each col as c}
                <div class="value">
                    <span>
                        {sqlToStr(c.value)}
                    </span>
                </div>
            {/each}
        </div>
    {/each}
</div>

<style>
    #query-results {
        flex: 1;
        max-width: var(--width-inner);
        min-width: 16rem;
        overflow: auto;
        display: flex;
    }

    .col > div:nth-child(odd) {
        background: rgba(0, 0, 0, .1);
    }

    .head {
        background: rgba(0, 0, 0, .15);
    }

    .head > b, .value > span {
        margin: 0 .33rem;
    }

    .col {
        display: flex;
        flex-direction: column;
        max-width: 10rem;
        word-wrap: break-word;
        border-right: 1px solid rgba(0, 0, 0, .1);
    }
</style>
