<script lang="ts">
    import {ITableView} from "$lib/types/table";

    let {view, selectedView = $bindable()}: {
        view: ITableView,
        selectedView: ITableView
    } = $props();

    function onClick() {
        selectedView = view;
    }
</script>

<div
        class={selectedView === view ? 'selected' : ''}
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
        color: var(--col-s-a);
        background: var(--col-text);
        text-align: center;
        cursor: pointer;
    }

    div:hover {
        border-bottom: 2px solid var(--col-s);
    }

    .selected {
        background: var(--col-s);
        color: var(--col-text);
        border-bottom: 2px solid var(--col-s);
    }
</style>