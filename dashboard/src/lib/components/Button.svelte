<script lang="ts">
    import Loading from "./Loading.svelte";
    import {fade} from "svelte/transition";

    import type {Snippet} from 'svelte'

    let {
        type = 'button',
        role = 'button',
        ref = $bindable(),
        id,
        ariaLabel,
        ariaControls,
        ariaCurrent,
        level = 2,
        width,
        isDisabled = false,
        isLoading = false,
        destructive = false,
        invisible = false,
        invisibleOutline = false,
        popovertarget,
        popovertargetaction,
        onclick,
        onLeft,
        onRight,
        onUp,
        onDown,
        children,
        ...rest
    }: {
        type?: "button" | "submit" | "reset" | null | undefined,
        role?: string,
        ref?: undefined | HTMLButtonElement,
        id?: string,
        ariaLabel?: string,
        ariaControls?: string,
        ariaCurrent?: "time" | "page" | "step" | "location" | "date" | undefined,
        level?: number,
        width?: string,
        isDisabled?: boolean,
        isLoading?: boolean,
        destructive?: boolean,
        invisible?: boolean,
        invisibleOutline?: boolean,
        popovertarget?: string,
        popovertargetaction?: 'toggle' | 'show' | 'hide' | null | undefined,
        children: Snippet,

        onclick?: (ev: Event) => void,
        onLeft?: () => void,
        onRight?: () => void,
        onUp?: () => void,
        onDown?: () => void,
    } = $props();

    let cls = $derived.by(() => {
        if (invisible) {
            return 'invisible';
        }
        if (destructive) {
            return 'destructive';
        }
        switch (level) {
            case 2:
                return 'l2';
            case 3:
                return 'l3';
            default:
                return 'l1';
        }
    });
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

    function loadingColor() {
        if (destructive) {
            return 'var(--btn-text)';
        }
        switch (level) {
            case 2:
                return 'hsl(var(--action))';
            case 3:
                return 'hsl(var(--action))';
            default:
                return 'var(--btn-text)';
        }
    }

    function onkeydown(ev: KeyboardEvent) {
        switch (ev.code) {
            case 'Enter':
                onclick?.(ev);
                break;
            case 'ArrowLeft':
                onLeft?.();
                break;
            case 'ArrowRight':
                onRight?.();
                break;
            case 'ArrowUp':
                onUp?.();
                break;
            case 'ArrowDown':
                onDown?.();
                break;
        }
    }

</script>

<button
        bind:this={ref}
        {role}
        {type}
        {id}
        aria-label={ariaLabel}
        aria-controls={ariaControls}
        aria-current={ariaCurrent}
        class={cls}
        class:invisibleOutline
        style:width
        data-isloading={isLoading}
        {onclick}
        {onkeydown}
        {disabled}
        aria-disabled={disabled}
        {popovertarget}
        {popovertargetaction}
        {...rest}
>
    {#if isLoading}
        <div class="load">
            <Loading background={false} color={loadingColor()}/>
        </div>
    {:else if showText}
        <div in:fade class="font-label">
            {@render children()}
        </div>
    {/if}
</button>

<style>
    button {
        height: 30px;
        margin: 5px;
        padding: 0 10px;
        font-weight: bold;
        font-size: .9rem;
        outline: none;
        border: none;
        border-radius: var(--border-radius);
        cursor: pointer;
        transition: all 150ms;
    }

    button:hover {
        box-shadow: 1px 1px 2px hsl(var(--bg-high));
    }

    button:focus-visible {
        outline: 2px solid hsl(var(--accent));
    }

    .destructive {
        color: var(--btn-text);
        background: hsl(var(--error));
    }

    .invisible, .invisible:hover {
        margin: 0;
        padding: 0;
        outline: none;
        background: none;
        box-shadow: none;
        color: hsl(var(--action));
    }

    .invisibleOutline:focus {
        outline: none;
    }

    button[aria-disabled="true"],
    button[aria-disabled="true"]:hover,
    button[aria-disabled="true"]:focus {
        color: hsl(var(--bg-high));
    }

    button[aria-disabled="true"],
    button[data-isloading="true"] {
        cursor: not-allowed;
    }

    .l1, .l2, .l3 {
        border: 1px solid hsla(var(--action), .5);
    }

    .l1 {
        color: var(--btn-text);
        background: hsl(var(--action));
    }

    .l2 {
        color: hsl(var(--action));
        border: 1px solid hsl(var(--action));
        background: transparent;
    }

    .l3 {
        color: hsl(var(--action));
        border: none;
        background: transparent;
    }

    .load {
        display: flex;
        justify-content: center;
    }
</style>
