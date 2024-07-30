<script lang="ts">
    import Loading from "./Loading.svelte";
    import {fade} from "svelte/transition";

    let {
        type = 'button',
        level = 1,
        width = 'inherit',
        isDisabled = false,
        isLoading = false,
        onclick,
    }: {
        type?: "button" | "submit" | "reset" | null | undefined,
        level?: number,
        width?: string,
        isDisabled?: boolean,
        isLoading?: boolean,
        onclick?: () => void,
    } = $props();

    let showText = $state(!isLoading);
    let disabled = $derived(isDisabled || isLoading);

    $effect(() => {
        if (isLoading) {
            setTimeout(() => {
                showText = false;
            }, 120);
        } else {
            setTimeout(() => {
                showText = true;
            }, 120);
        }
    })

    function handleCLick() {
        if (onclick) {
            onclick();
        }
    }
</script>

<button
        type={type}
        class:l1={level === 1}
        class:l2={level === 2}
        class:l3={level > 2}
        style:width={width}
        style:cursor="{isLoading ? 'default' : 'pointer'}"
        onclick={handleCLick}
        onkeydown={handleCLick}
        { disabled }
>
    {#if isLoading}
        <div class="load">
            <Loading
                    background={false}
                    color={level > 1 ? 'var(--col-text)' : 'var(--col-text)'}
            />
        </div>
    {:else if showText}
        <div in:fade class="txt">
            <slot/>
        </div>
    {/if}
</button>

<style>
    button {
        height: 30px;
        margin: 5px;
        padding: 0 10px;
        font-weight: bold;
        outline: none;
        border-radius: 3px;
        transition: all 150ms;
    }

    button:focus {
        outline: 1px solid var(--col-btn);
    }

    .l1, .l2, .l3 {
        border: 1px solid var(--col-mid);
        box-shadow: 1px 1px 2px var(--col-mid);
    }

    .l1 {
        color: var(--col-btn-acnt);
        background: var(--col-btn);
    }

    .l1:hover {
        box-shadow: 2px 2px 4px var(--col-mid);
    }

    .l2 {
        color: var(--col-btn);
        border: 1px solid var(--col-btn);
        background: var(--col-bg);
    }

    .l2:hover {
        box-shadow: 2px 2px 4px var(--col-mid)
    }

    .l3 {
        color: var(--col-btn);
        border: none;
        background: var(--col-bg);
    }

    .l3:hover {
        box-shadow: 2px 2px 4px var(--col-mid)
    }

    .load {
        display: flex;
        justify-content: center;
    }

    .txt {
        margin-top: 1px;
    }
</style>
