<script lang="ts">
    import Button from "$lib/components/Button.svelte";
    import {untrack} from "svelte";
    import Options from "$lib/components/Options.svelte";
    import IconChevronRight from "$lib/components/icons/IconChevronRight.svelte";

    const options = [5, 7, 10, 15, 20, 30, 50, 100];

    let {
        items = $bindable(),
        itemsPaginated = $bindable(),
        page = $bindable(1),
        pageSize = $bindable(options[0]),
        compact = false,
    }: {
        items: any[];
        itemsPaginated: any[];
        page?: number;
        pageSize?: number;
        compact?: boolean;
    } = $props();

    const iconSize = 16;
    let pageSizeBefore = untrack(() => pageSize);

    let itemsArr: any[] = $state([]);
    let pageLinks: number[] = $state([]);

    $effect(() => {
        // make sure to reset and go to page 1 if chunk size changes
        if (pageSize !== pageSizeBefore) {
            pageSizeBefore = untrack(() => pageSize);
            page = 1;
        }
    });

    $effect(() => {
        let res = [];
        for (let i = 0; i < items.length; i += pageSize) {
            const chunk = items.slice(i, i + pageSize);
            res.push(chunk);
        }
        itemsArr = res;
        itemsPaginated = res[page - 1];
    });

    $effect(() => {
        calculate();
    });

    function goLeft() {
        if (page > 1) {
            goto(page - 1);
        }
    }

    function goRight() {
        if (page < itemsArr.length) {
            goto(page + 1);
        }
    }

    function goto(newPage: number) {
        page = newPage;
        calculate();
    }

    function calculate() {
        let links = [];
        let countHalf = Math.floor(pageSize / 2);

        if (itemsArr.length <= pageSize) {
            for (let i = 1; i <= itemsArr.length; i++) {
                links.push(i);
            }

        } else if (page <= countHalf) {
            for (let i = 1; i <= pageSize; i++) {
                links.push(i);
            }

        } else if (page > itemsArr.length - countHalf - 1) {
            for (let i = itemsArr.length - pageSize; i <= itemsArr.length - 1; i++) {
                links.push(i + 1);
            }

        } else {
            for (let i = page - countHalf; i < page - countHalf + pageSize; i++) {
                links.push(i);
            }
        }

        pageLinks = links;
    }

</script>

{#snippet links()}
    <nav aria-label="Pagination">
        <ul>
            {#each pageLinks as no}
                <li
                        aria-label={`go to page number: ${no}`}
                        aria-current={page === no ? 'step' : undefined}
                >
                    <Button
                            invisible
                            onclick={() => goto(no)}
                            onLeft={goLeft}
                            onRight={goRight}
                    >
                        <div class="link noselect">
                            {no}
                        </div>
                    </Button>
                </li>
            {/each}
        </ul>
    </nav>
{/snippet}

{#snippet chunkSizeTotal()}
    <div class="flex gap-10">
        <div class="flex gap-05 chunkSize noselect">
            <div>
                Entries
            </div>
            <div>
                <Options
                        ariaLabel="Page Count"
                        bind:value={pageSize}
                        options={options}
                        offsetTop="-17rem"
                        borderless
                />
            </div>
        </div>
        <div class="font-label total">Total: {items.length}</div>
    </div>
{/snippet}

<div class="container">
    <Button onclick={goLeft} invisible isDisabled={page === 1}>
        <div class="iconLeft" aria-label="Go to previous page" data-disabled={page === 1}>
            <IconChevronRight width={iconSize}/>
        </div>
    </Button>

    {@render links()}

    <Button onclick={goRight} invisible isDisabled={page === itemsArr.length}>
        <div class="iconRight" aria-label="Go to next page" data-disabled={page === itemsArr.length}>
            <IconChevronRight width={iconSize}/>
        </div>
    </Button>

    {#if !compact}
        {@render chunkSizeTotal()}
    {/if}
</div>

<style>
    nav {
        display: flex;
        margin: 0 .8rem 0 .25rem;
    }

    ul {
        list-style-type: none;
        margin: 0;
        padding: 0;
        display: inline-flex;
    }

    li[aria-current="step"] div {
        color: hsla(var(--action), .65);
        font-weight: bold;
        text-decoration: underline;
    }

    .chunkSize {
        margin-left: 10px;
    }

    .container {
        display: flex;
        align-items: center;
        padding: .25rem .5rem;
    }

    .iconLeft,
    .iconRight {
        position: relative;
        bottom: -.1rem;
        color: hsla(var(--action), .7);
    }

    .iconLeft[data-disabled="true"],
    .iconRight[data-disabled="true"] {
        color: hsl(var(--bg-high));
    }

    .iconLeft {
        margin: -5px 0 0 0;
        transform: rotate(180deg);
    }

    .iconRight {
        margin: 0 0 -3px -8px;
    }

    .link {
        width: 1.2rem;
        margin: 2px;
        padding: 2px;
        cursor: pointer;
        color: hsla(var(--action), .5);
    }

    .total {
        transform: translateY(.05rem);
        text-wrap: nowrap;
    }
</style>
