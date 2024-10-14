<script lang="ts">
    import Button from "$lib/components/Button.svelte";
    import {copyToClip} from "$lib/utils/copyToClip";
    import A from "$lib/components/A.svelte";
    import Pagination from "$lib/components/Pagination.svelte";
    import Checkbox from "$lib/components/Checkbox.svelte";
    import Popover from "$lib/components/Popover.svelte";
    import {untrack} from "svelte";
    import IconChevronDown from "$lib/components/icons/IconChevronDown.svelte";
    import IconArrowUpDown from "$lib/components/icons/IconArrowUpDown.svelte";
    import IconCog from "$lib/components/icons/IconCog.svelte";
    import CheckIcon from "$lib/components/CheckIcon.svelte";
    import IconDotsHorizontal from "$lib/components/icons/IconDotsHorizontal.svelte";
    import IconEye from "$lib/components/icons/IconEye.svelte";

    import type {IDataTable, IRow} from "$lib/components/data_table/data_table_types";

    let {
        caption,
        columns,
        showColumns = $bindable(Array(columns.length).fill(true)),
        rows = $bindable(),
        options,
        offsetLeftOptions,
        offsetTopOptions,
        offsetLeftColumnSelect,
        paginationCompact = false,
        select,
        width,
        maxWidth,
        minWidthColPx = 50,
    }: IDataTable = $props();

    const SELECT_WIDTH = '3rem';
    const OPTION_WIDTH = '2rem';

    // make sure gridTemplateColumns is valid with selected options
    let columnWidths = $state(buildInitialColumnWidths());
    let columnWidthsSelected = $state(untrack(() => columnWidths));

    let page = $state(1);
    let pageSize = $state(15);
    let selectedRows = $state(Array(rows.length).fill(false));
    let isAnySelected = $derived(selectedRows.find(r => r === true));

    let closePopoverSelect: undefined | (() => void) = $state();
    let closePopoverOption: (() => void)[] = $state([]);

    $inspect(rows, columns).with(() => {
        if (rows.length > 0 && columns.length !== rows[0].length) {
            console.warn('`columns` and `entries` have different lengths', columns, rows);
        } else {
            console.log('`columns` and `entries` lengths match');
        }
    })

    let checkedAll = $state(false);
    let rowsPaginated: IRow[][] = $state([]);
    let orderDir: 'up' | 'down' = $state('up');

    let rowCount = $derived.by(() => {
        if (rowsPaginated && rowsPaginated.length) {
            return rowsPaginated.length;
        }
        if (page != 1) {
            page = 1;
        }
        return 0;
    });

    let refCols: (undefined | HTMLElement)[] = $state(Array(untrack(() => columnWidths.length)).fill(undefined));
    let resizingCol = 0;

    // If there are any `auto` columns in the template, we need to convert them to absolute numbers
    // to make the resizing not completely weird to use.
    setTimeout(() => {
        for (let i = 1; i <= columnWidths.length; i++) {
            if (columnWidths[i] === 'auto') {
                resizingCol = i - 1;
                let ref = refCols[i];
                if (ref) {
                    updateColSize(ref.getBoundingClientRect().width);
                }
            }
        }
    }, 1000);

    $effect(() => {
        let newSel = Array(rows.length).fill(false);

        // to only select "all" that are currently in the view
        if (checkedAll) {
            let from;
            if (page === 1) {
                from = 0;
            } else {
                from = (page - 1) * pageSize
            }
            let until = Math.min(page * pageSize, rows.length);

            for (let i = from; i < until; i++) {
                newSel[i] = true;
            }
        }

        selectedRows = newSel;
    });

    $effect(() => {
        closePopoverOption = Array(rowsPaginated?.length).fill(() => console.error('un-initialized popover close option'));
    });

    $effect(() => {
        let newWidths = [];
        for (let i = 0; i < columnWidths.length; i++) {
            if (showColumns[i]) {
                newWidths.push(columnWidths[i]);
            }
        }
        columnWidthsSelected = newWidths;
    });

    function buildInitialColumnWidths() {
        let widths = columns.map(col => col.initialWidth);

        if (select) {
            widths = [SELECT_WIDTH, ...widths];
        }
        if (options) {
            widths = [...widths, OPTION_WIDTH];
        }

        showColumns = Array(widths.length).fill(true);
        return widths;
    }

    function gridTemplateColumns() {
        return columnWidthsSelected.join(' ');
    }

    function orderBy(col: number, typ: 'string' | 'number' | undefined) {
        selectedRows = Array(rows.length).fill(false);

        let mod = 1;
        if (orderDir === 'up') {
            mod = -1;
            orderDir = 'down';
        } else {
            orderDir = 'up';
        }

        if (typ === 'string') {
            rows.sort((a, b) => (a[col].content as string).localeCompare(b[col].content as string) * mod);
        } else if (typ === 'number') {
            rows.sort((a, b) => ((a[col].content as number) - (b[col].content as number)) * mod);
        }
    }

    function absoluteRowNo(rowNo: number) {
        if (page > 1) {
            return (page - 1) * pageSize + rowNo;
        } else {
            return rowNo;
        }
    }

    function onMouseDown(col: number) {
        resizingCol = col;
        let ref = refCols[col];
        if (ref) {
            updateColSize(ref.getBoundingClientRect().width);
            window.addEventListener('mousemove', onMove);
            window.addEventListener('mouseup', onMouseUp, {once: true});
        } else {
            console.error('invalid ref from refCols in onMouseDown');
        }
    }

    function onMouseUp() {
        window.removeEventListener('mousemove', onMove);
    }

    function onMove(ev: MouseEvent) {
        let ref = refCols[resizingCol];
        if (ref) {
            let left = ref.getBoundingClientRect().left;
            let width = window.scrollX + ev.x - left;
            updateColSize(width);
        } else {
            console.error('invalid ref from refCols in onMove');
        }
    }

    function updateColSize(width: number) {
        width = Math.ceil(width);
        if (width < minWidthColPx) {
            width = minWidthColPx;
        }
        columnWidths[select ? resizingCol + 1 : resizingCol] = `${width}px`;
    }
</script>

<table
        aria-colcount={columnWidths.length}
        aria-rowcount={rowCount}
        style:width
        style:max-width={maxWidth}
>
    <thead>
    <tr style:grid-template-columns={gridTemplateColumns()}>
        {#if select && showColumns[0]}
            <th class="headerCheckbox">
                <Checkbox
                        ariaLabel="Select All"
                        bind:checked={checkedAll}
                        borderColor="hsla(var(--text), .4)"
                />

                <Popover
                        ariaLabel="Options for the selection"
                        bind:close={closePopoverSelect}
                        btnDisabled={!isAnySelected}
                        btnInvisible
                >
                    {#snippet button()}
                        <span class="btnSelect" data-disabled={!isAnySelected}>
                            <IconChevronDown width={18}/>
                        </span>
                    {/snippet}
                    {@render select(selectedRows, closePopoverSelect)}
                </Popover>
            </th>
        {/if}

        {#each columns as column, i}
            {#if showColumns[select ? i + 1 : i]}
                <th bind:this={refCols[i]}>
                    <span class="flex-1 label">
                        {#if column.orderType}
                            {column.content}
                            <Button invisible onclick={() => orderBy(i, column.orderType)}>
                                <span class="iconOrder">
                                    <IconArrowUpDown width={16}/>
                                </span>
                            </Button>
                        {:else}
                            <span class="rawText">
                                {column.content}
                            </span>
                        {/if}
                    </span>

                    <span class="relative">
                    <span role="none" class="absolute sizeable" onmousedown={() => onMouseDown(i)}>
                    </span>
                </span>
                </th>
            {/if}
        {/each}

        {#if options && showColumns[showColumns.length - 1]}
            <th class="headerOptions">
                <IconCog width={20}/>
            </th>
        {/if}
    </tr>
    </thead>

    <tbody>
    {#each rowsPaginated as row, i}
        <tr style:grid-template-columns={gridTemplateColumns()}>
            {#if select && showColumns[0]}
                <td class="checkbox">
                    <Checkbox ariaLabel="Select Row" bind:checked={selectedRows[absoluteRowNo(i)]}/>
                </td>
            {/if}

            {#each row as column, j}
                {#if showColumns[select ? j + 1 : j]}
                    <td>
                        {#if columns[j]?.showAs === 'a'}
                            <A href={column.href || ''}>
                            <span class="linkText">
                                {column.content}
                            </span>
                            </A>
                        {:else if columns[j]?.showAs === 'a_blank'}
                            <A href={column.href || ''} target="_blank">
                            <span class="linkText">
                                {column.content}
                            </span>
                            </A>
                        {:else if columns[j]?.showAs === 'copyToClip'}
                            <Button invisible onclick={() => copyToClip(column.content.toString())}>
                            <span class="copyToClip">
                                {column.content}
                            </span>
                            </Button>
                        {:else if columns[j]?.showAs === 'check'}
                        <span class="checkIcon">
                            <CheckIcon checked={column.content}/>
                        </span>
                        {:else if column.onClick}
                            <Button invisible onclick={ev => column.onClick?.(ev, absoluteRowNo(i))}>
                            <span class="onclick">
                                {column.content}
                            </span>
                            </Button>
                        {:else}
                        <span class="rawText">
                            {column.content}
                        </span>
                        {/if}
                    </td>
                {/if}
            {/each}

            {#if options && showColumns[showColumns.length - 1]}
                <td class="options">
                    <Popover
                            ariaLabel="Options"
                            bind:close={closePopoverOption[i]}
                            btnInvisible
                            offsetLeft={offsetLeftOptions}
                            offsetTop={offsetTopOptions}
                    >
                        {#snippet button()}
                            <span class="btnOptions">
                                <IconDotsHorizontal/>
                            </span>
                        {/snippet}
                        {@render options(row, closePopoverOption[i])}
                    </Popover>
                </td>
            {/if}
        </tr>
    {/each}
    </tbody>

    <caption class="flex space-between">
        <Pagination
                items={rows}
                bind:itemsPaginated={rowsPaginated}
                bind:page
                bind:pageSize
                compact={paginationCompact}
        />

        <span class="flex gap-05">
            <span>
                {caption}
            </span>
            <Popover
                    ariaLabel="Select Columns"
                    offsetLeft={offsetLeftColumnSelect || '-6rem'}
                    offsetTop={`-${columnWidths.length * 1.4 + 2.7}rem`}
                    btnInvisible
            >
                {#snippet button()}
                    <span class="eye">
                        <IconEye/>
                    </span>
                {/snippet}
                <div class="columnsSelect">
                    {#if select}
                        <div class="columnSelect">
                            <Checkbox
                                    ariaLabel="Select Columns: Select"
                                    bind:checked={showColumns[0]}
                            >
                                Select
                            </Checkbox>
                        </div>
                    {/if}

                    {#each columns as column, i}
                        <div class="columnSelect">
                            <Checkbox
                                    ariaLabel={`Select Columns: ${column.content}`}
                                    bind:checked={showColumns[select ? i + 1 : i]}
                            >
                                {column.content}
                            </Checkbox>
                        </div>
                    {/each}

                    {#if options}
                        <div class="columnSelect">
                            <Checkbox
                                    ariaLabel="Select Columns: Options"
                                    bind:checked={showColumns[showColumns.length - 1]}
                            >
                                Options
                            </Checkbox>
                        </div>
                    {/if}
                </div>
            </Popover>
        </span>
    </caption>
</table>

<style>
    caption {
        padding-right: .5rem;
        caption-side: bottom;
        text-align: left;
    }

    caption > span {
        color: hsl(var(--bg-high));
    }

    table {
        width: 100%;
    }

    thead {
        display: block;
    }

    thead tr {
        background: hsl(var(--bg-high));
        border-radius: var(--border-radius) var(--border-radius) 0 0;
    }

    tbody {
        display: block;
        overflow-y: scroll;
    }

    tbody tr:last-child {
        border-radius: 0 0 var(--border-radius) var(--border-radius);
    }

    tr {
        margin: 1px 0;
        display: grid;
        overflow: clip;
    }

    tbody tr {
        transition: background 150ms;
    }

    td, th {
        text-align: left;
        padding: .1rem .5rem;
    }

    th {
        display: flex;
        color: hsl(var(--text-high));
        font-weight: normal;
    }

    th .label {
        overflow-x: scroll;
    }

    tbody td {
        text-wrap: wrap;
        word-wrap: break-word;
        vertical-align: text-top;
        overflow-x: scroll;
    }

    tbody tr:nth-child(even) {
        background: hsla(var(--bg-high), .2);
    }

    tbody tr:hover {
        background: hsla(var(--accent), .33);
    }

    .btnOptions {
        color: hsl(var(--text));
        transition: color 150ms;
    }

    .btnOptions:hover {
        color: hsl(var(--action));
    }

    .btnSelect {
        color: hsl(var(--action));
        transition: color 150ms;
    }

    .btnSelect[data-disabled="true"] {
        color: hsla(var(--text), .5);
    }

    .checkbox {
        transform: translateY(.1rem);
    }

    .checkIcon {
        position: relative;
        bottom: -.2rem;
    }

    .columnsSelect {
        padding: .5rem;
        /*border: 1px solid red;*/
    }

    .columnSelect {
        height: 1.4rem;
        overflow: clip;
    }

    .copyToClip {
        position: relative;
        top: -.05rem;
        color: hsl(var(--text));
        font-size: 1rem;
        font-weight: normal;
        cursor: copy;
        overflow: scroll;
    }

    .eye {
        position: relative;
        bottom: -.15rem;
        color: hsla(var(--text), .5);
        transition: all 150ms;
    }

    .eye:hover {
        color: hsl(var(--action));
    }

    .headerCheckbox {
        display: grid;
        grid-template-columns: 1.3rem 1rem;
        transform: translateY(.2rem);
    }

    .headerOptions {
        height: 100%;
        padding: 0;
        justify-content: center;
        margin: auto 0;
        color: hsla(var(--text), .66);
    }

    .iconOrder {
        position: relative;
        bottom: -.15rem;
        color: hsla(var(--text), .7);
        transition: all 150ms;
    }

    .iconOrder:hover {
        color: hsl(var(--action));
    }

    .linkText {
        position: relative;
        bottom: -.05rem;
    }

    .onclick {
        color: hsla(var(--action), .75);
        transition: color 150ms;
    }

    .onclick:hover {
        color: hsl(var(--action));
    }

    .options {
        margin: 0;
        padding: 0;
        text-align: center;
        transform: translateY(.1rem);
    }

    .rawText {
        position: relative;
        bottom: -.15rem;
    }

    .sizeable {
        left: 4px;
        height: 100%;
        width: 8px;
        background: transparent;
        cursor: col-resize;
        transition: all 150ms;
        border-right: 1px solid hsla(var(--text), .15);
    }

    .sizeable:hover {
        background: hsla(var(--text), .33);
    }
</style>
