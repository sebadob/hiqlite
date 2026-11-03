<script lang="ts">
    import type { IColumn, IRow } from '#lib/types/query_results.js';
    import { sqlValueDisplay } from '#lib/utils/sqlValue.js';

    let { rows = $bindable() }: { rows: IRow[] } = $props();
    $inspect(rows);

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
    });
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
                        {sqlValueDisplay(c.value)}
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
        border: 1px solid red;
    }

    .col > div:nth-child(odd) {
        background: rgba(0, 0, 0, 0.1);
    }

    .head {
        background: rgba(0, 0, 0, 0.15);
    }

    .head > b,
    .value > span {
        margin: 0 0.33rem;
    }

    .col {
        display: flex;
        flex-direction: column;
        border-right: 1px solid rgba(0, 0, 0, 0.1);
    }
</style>
