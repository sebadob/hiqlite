<script lang="ts">
    import Popover from "$lib/components/Popover.svelte";
    import Button from "$lib/components/Button.svelte";
    import SearchBar from "$lib/components/SearchBar.svelte";
    import IconChevronRight from "$lib/components/icons/IconChevronRight.svelte";

    let {
        ref = $bindable(),
        ariaLabel,
        options = [],
        name,
        value = $bindable(),
        maxHeight,
        offsetTop,
        offsetLeft,
        asPopover = true,
        borderless = false,
        withSearch = false,
        fallbackOptions = false,
        onLeft,
        onRight,
        onUp,
        onDown,
    }: {
        ref?: undefined | HTMLButtonElement,
        ariaLabel: string,
        options: string[] | number[],
        name?: string,
        value?: string | number,
        maxHeight?: string,
        offsetTop?: string,
        offsetLeft?: string,
        asPopover?: boolean,
        borderless?: boolean,
        withSearch?: boolean,
        fallbackOptions?: boolean,
        onLeft?: () => void,
        onRight?: () => void,
        onUp?: () => void,
        onDown?: () => void,
    } = $props();

    $inspect(options).with(() => {
        if (options.length > 0 && typeof options[0] !== typeof value) {
            console.error("type of 'options' does not match the one of 'value'");
            console.log(options);
            console.log(value);
        }
    });

    let refOptions: undefined | HTMLElement = $state();
    let usePopover = $state(fallbackOptions ? false : asPopover);
    let close: undefined | (() => void) = $state();

    let selected = $state(withSearch ? -1 : 0);
    let focusSearch: undefined | ((options?: FocusOptions) => void) = $state();

    let searchValue = $state('');
    let optionsFiltered = $derived.by(() => {
        if (!withSearch) {
            return options;
        }

        if (typeof value === 'string') {
            return options.filter(opt => (opt as string).toLowerCase().includes(searchValue.toLowerCase()));
        }

        let searchInt = Number.parseInt(searchValue) || value;
        return options.filter(opt => opt === searchInt);
    });

    $effect(() => {
        // wrapped inside an effect to have the default options as fallback
        if (usePopover !== asPopover) {
            usePopover = asPopover;
        }
    });

    $effect(() => {
        if (selected === -1) {
            refOptions?.scrollTo({
                top: 0,
                behavior: "smooth",
            });
        }

        if (withSearch) {
            if (selected < 0 || selected > optionsFiltered.length - 1) {
                selected = -1;
                focusSearch?.();
                return;
            }
        } else {
            if (selected < 0) {
                selected = optionsFiltered.length - 1;
            } else if (selected > optionsFiltered.length - 1) {
                selected = 0;
            }
            focusCurrElem();
        }
    });

    function focusCurrElem() {
        if (refOptions) {
            let button = refOptions.getElementsByTagName('button')[selected];
            button.scrollIntoView({behavior: 'smooth', block: 'center'});
            button.focus();
        } else {
            console.error('refOptions is undefined');
        }
    }

    function onToggle(newState: string) {
        if (newState === 'open') {
            if (withSearch) {
                selected = -1;
                focusSearch?.();
            } else {
                selected = options.findIndex(opt => opt === value) || 0;
                focusCurrElem();
            }
        }
    }

    function onkeydown(ev: KeyboardEvent) {
        let code = ev.code;

        if (code === 'ArrowDown') {
            ev.preventDefault();
            if (hasFilteredItems()) {
                selected += 1;
            }
        } else if (code === 'ArrowUp') {
            ev.preventDefault();
            if (hasFilteredItems()) {
                selected -= 1;
            }
        } else if (code === 'Enter' && selected > -1) {
            select(optionsFiltered[selected]);
        } else if (code === 'Enter' && selected === -1 && optionsFiltered.length === 1) {
            select(optionsFiltered[0]);
        }
    }

    function hasFilteredItems() {
        if (optionsFiltered.length > 0) {
            return true;
        }
        selected = -1;
        return false;
    }

    function select(option: string | number) {
        value = option;
        searchValue = '';

        setTimeout(() => {
            close?.();
        }, 20);
    }
</script>

{#if usePopover}
    <Popover
            bind:ref
            {ariaLabel}
            roleButton="combobox"
            btnInvisible
            bind:close
            {offsetTop}
            {offsetLeft}
            {onToggle}
            {onLeft}
            {onRight}
            {onUp}
            {onDown}
    >
        {#snippet button()}
            <div class="btn" data-border={!borderless}>
                {value}
                <div class="chevron">
                    <IconChevronRight width={14}/>
                </div>
            </div>
        {/snippet}

        <div role="listbox" tabindex="0" class="popover" style:max-height={maxHeight} {onkeydown}>
            {#if withSearch}
                <SearchBar
                        bind:value={searchValue}
                        bind:focus={focusSearch}
                        onFocus={() => selected = -1}
                />
            {/if}

            <div bind:this={refOptions} class="popoverOptions">
                {#each optionsFiltered as option, i}
                    <Button invisible invisibleOutline onclick={() => select(option)}>
                        <div
                                class="optPopover"
                                aria-selected={value === option}
                                data-focus={selected === i}
                        >
                            {option}
                        </div>
                    </Button>
                {/each}
            </div>
        </div>
    </Popover>
{:else}
    <select
            name={name}
            aria-label={ariaLabel}
            class:borderless
            bind:value
    >
        {#each optionsFiltered as opt}
            <option class="opt" value={opt} selected={value === opt}>
                {opt}
            </option>
        {/each}
    </select>
{/if}

<style>
    select {
        padding: 2px 1px 5px 9px;
        color: hsl(var(--text));
        font-size: .95rem;
        cursor: pointer;
        border-radius: var(--border-radius);
        border: 1px solid hsl(var(--bg-high));
        outline: none;
        background: transparent;
        transition: all 125ms ease-in-out;
    }

    select:hover {
        color: hsl(var(--action));
        border: 1px solid hsl(var(--action));
    }

    .borderless, .borderless:hover {
        border: none;
    }

    .btn {
        display: inline-flex;
        gap: .25rem;
        color: hsl(var(--text));
        border-radius: var(--border-radius);
        font-weight: normal;
        font-size: .9rem;
        transition: all 150ms;
    }

    .btn[data-border="true"] {
        padding: .15rem .33rem .3rem .5rem;
        border: 1px solid hsl(var(--bg-high));
    }

    .btn:hover {
        color: hsl(var(--action));
    }

    .chevron {
        transform: rotate(90deg);
    }

    .opt {
        color: hsl(var(--text));
        cursor: pointer;
    }

    .optPopover {
        text-align: left;
        padding: .1rem .5rem;
        color: hsl(var(--text));
        font-size: .95rem;
        font-weight: normal;
        cursor: pointer;
        transition: all 150ms;
    }

    .optPopover[aria-selected="true"] {
        color: hsl(var(--text-high));
    }

    .optPopover:hover {
        color: hsl(var(--action));
    }

    .optPopover[data-focus="true"] {
        color: hsl(var(--action));
        background: hsl(var(--bg-high));
    }

    .popover {
        display: flex;
        flex-direction: column;
        align-items: flex-start;
    }

    .popoverOptions {
        height: 100%;
        display: flex;
        flex-direction: column;
        overflow-y: auto;
    }
</style>
