<script lang="ts">
    import IconStop from "$lib/components/icons/IconStop.svelte";

    import type {Snippet} from "svelte";

    let {
        tab = $bindable(),
        tabSelected = $bindable(),
        onClose,
        children,
    }: {
        tab: string,
        tabSelected: string
        onClose: (id: string) => void,
        children: Snippet,
    } = $props();

    let ref: HTMLDivElement;
    let isSelected = $derived(tabSelected === tab);

    function onkeydown(ev: KeyboardEvent) {
        if (isSelected) {
            if (ev.code === 'Enter') {
                ev.preventDefault();
                onblur();
            }
        } else {
            onclick();
        }
    }

    function onblur() {
        let v = ref.innerText;
        tab = v;
        tabSelected = v;
    }

    function onclick() {
        if (!isSelected) {
            tabSelected = tab;
        }
    }

    function close() {
        onClose(tab);
    }
</script>

<div class="row">
    <div
            bind:this={ref}
            class={isSelected ? 'tab selected' : 'tab'}
            contenteditable={isSelected}
            role="button"
            tabindex="0"
            {onclick}
            {onkeydown}
            {onblur}
    >
        {@render children()}
    </div>
    <div class="close">
        <div
                role="button"
                tabindex="0"
                class="close-inner"
                onclick={close}
                onkeydown={close}
        >
            <IconStop color="hsl(var(--error))"/>
        </div>
    </div>
</div>

<style>
    .row {
        display: flex;
    }

    .tab {
        padding-left: 5px;
        padding-right: 20px;
        color: hsl(var(--action));
        background: hsl(var(--bg-high));
        border-bottom: 2px solid hsl(var(--bg-high));
        text-align: center;
        cursor: pointer;
    }

    .tab:hover {
        border-bottom: 2px solid hsl(var(--action));
    }

    .selected {
        background: hsl(var(--action));
        color: hsl(var(--bg));
        border-bottom: 2px solid hsl(var(--action));
        cursor: text;
    }

    .close {
        position: relative;
    }

    .close-inner {
        position: absolute;
        top: 0;
        right: 0;
        cursor: pointer;
    }
</style>
