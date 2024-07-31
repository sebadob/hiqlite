<script lang="ts">

    import IconStop from "$lib/components/icons/IconStop.svelte";

    let {
        tab = $bindable(),
        tabSelected = $bindable(),
        onClose,
    }: {
        tab: string,
        tabSelected: string
        onClose: (id: string) => void,
    } = $props();

    let ref: HTMLDivElement;
    let isSelected = $derived(tabSelected === tab);

    function onKeyDown(ev: KeyboardEvent) {
        if (isSelected) {
            if (ev.code === 'Enter') {
                ev.preventDefault();
                saveName();
            }
        } else {
            onClick();
        }
    }

    function saveName() {
        let v = ref.innerText;
        tab = v;
        tabSelected = v;
    }

    function onClick() {
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
            onclick={onClick}
            onkeydown={onKeyDown}
            onblur={saveName}
    >
        <slot/>
    </div>
    <div class="close">
        <div
                role="button"
                tabindex="0"
                class="close-inner"
                onclick={close}
                onkeydown={close}
        >
            <IconStop/>
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
        color: var(--col-s-a);
        background: var(--col-text);
        text-align: center;
        cursor: pointer;
    }

    .tab:hover {
        border-bottom: 2px solid var(--col-s);
    }

    .selected {
        background: var(--col-s);
        color: var(--col-text);
        border-bottom: 2px solid var(--col-s);
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