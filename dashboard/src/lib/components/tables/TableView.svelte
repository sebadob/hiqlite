<script lang="ts">
    import {ITableView} from "$lib/types/table";

    let {view, viewSelected = $bindable()}: {
        view: ITableView,
        viewSelected: ITableView
    } = $props();

    function onClick() {
        viewSelected = view;
    }
</script>

<div
        class={viewSelected === view ? 'selected' : ''}
        role="button"
        tabindex="0"
        onclick={onClick}
        onkeydown={onClick}
>
    {view}
</div>

<style>
    div {
        flex: 1;
        background: hsl(var(--bg-high));
        color: hsl(var(--action));
        border-bottom: 2px solid hsl(var(--bg-high));
        text-align: center;
        cursor: pointer;
    }

    div:hover {
        border-bottom: 2px solid hsl(var(--action));
    }

    .selected {
        background: hsl(var(--action));
        color: hsl(var(--bg));
        border-bottom: 2px solid var(--col-p);
    }
</style>