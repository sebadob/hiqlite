<script lang="ts">
    import Button from "$lib/components/Button.svelte";
    import Options from "$lib/components/Options.svelte";
    import {genKey} from "$lib/utils/genKey";
    import IconBackspace from "$lib/components/icons/IconBackspace.svelte";
    import IconMagnify from "$lib/components/icons/IconMagnify.svelte";

    let {
        value = $bindable(''),
        datalist,
        options,
        option = $bindable(),
        focus = $bindable(),
        width = '100%',
        borderless,
        onSearch,
        onTab,
        onUp,
        onDown,
        onFocus,
    }: {
        value?: string;
        datalist?: string[];
        options?: string[];
        option?: string;
        focus?: undefined | ((options?: FocusOptions) => void);
        width?: string;
        borderless?: boolean;
        onSearch?: (value: string) => void,
        onTab?: (value: string) => void,
        onUp?: (value: string) => void,
        onDown?: (value: string) => void,
        onFocus?: () => void,
    } = $props();

    const idInput = genKey(8);
    const idDatalist = genKey(8);

    let ref: undefined | HTMLElement = $state();
    let list = $derived(datalist && datalist.length > 0 ? idDatalist : undefined);

    $effect(() => {
        focus = doFocus;
    });

    function onkeydown(ev: KeyboardEvent) {
        switch (ev.code) {
            case 'Enter':
                search();
                break;
            case 'Tab':
                onTab?.(value);
                break;
            case 'ArrowUp':
                onUp?.(value);
                break;
            case 'ArrowDown':
                onDown?.(value);
                break;
        }
    }

    function search() {
        onSearch?.(value);
    }

    function doFocus() {
        ref?.focus();
    }
</script>

<search
        class="flex container"
        style:border={borderless ? undefined : '1px solid hsl(var(--bg-high))'}
        style:width
>
    {#if options}
        <div class="options">
            <Options
                    ariaLabel="Search Options"
                    {options}
                    bind:value={option}
                    borderless
            />
        </div>
    {/if}

    <input
            bind:this={ref}
            type="search"
            id={idInput}
            {list}
            autocomplete="off"
            aria-label="Search"
            placeholder="Search"
            {onkeydown}
            onfocus={() => onFocus?.()}
            bind:value
    />

    {#if datalist}
        <datalist id={idDatalist} class="absolute">
            {#each datalist as value}
                <option {value}></option>
            {/each}
        </datalist>
    {/if}

    <div class="relative">
        <div class="absolute btnDelete">
            <Button ariaLabel="Delete Search Input" invisible onclick={() => value = ''}>
                <IconBackspace color="hsl(var(--bg-high))" width={24}/>
            </Button>
        </div>
    </div>

    {#if onSearch}
        <div class="btnSearch">
            <Button ariaLabel="Search" invisible onclick={search}>
                <div class="magnify">
                    <IconMagnify/>
                </div>
            </Button>
        </div>
    {/if}
</search>

<style>
    input {
        padding-right: 1.9rem;
        border: none;
        margin: 0;
    }

    datalist {
        display: block;
    }

    .btnDelete {
        top: -.9rem;
        right: .3rem;
        opacity: .8;
    }

    .btnSearch {
        margin: 2px 2px 2px 0;
        padding: 0 .25rem;
        background: hsl(var(--bg-high));
        border-left: 1px solid hsl(var(--bg-high));
        border-radius: 0 2px 2px 0;
    }

    .magnify {
        transform: translateY(.25rem);
    }

    .container {
        border-radius: var(--border-radius);
        overflow: clip;
    }

    .options {
        margin: 2px 0 2px 2px;
        padding: 0 .25rem;
        background: hsl(var(--bg-high));
        border-right: 1px solid hsl(var(--bg-high));
        border-radius: 2px 0 0 2px;
    }
</style>
