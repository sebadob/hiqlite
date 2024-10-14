<script lang="ts">
    import {slide} from "svelte/transition";

    import type {FullAutoFill} from "svelte/elements";
    import IconClipboard from "$lib/components/icons/IconClipboard.svelte";
    import IconEyeSlash from "$lib/components/icons/IconEyeSlash.svelte";
    import IconEye from "$lib/components/icons/IconEye.svelte";

    let {
        type = 'password',
        id,
        name = 'password',
        value = '',
        label = 'Password',
        autocomplete = 'current-password',
        placeholder = 'Password',
        title = 'Password',
        disabled = false,
        maxLength,
        min = '14',
        max = '128',
        required = true,
        pattern,
        width = 'inherit',

        showCopy = false,
    }: {
        type?: string,
        id?: undefined | string,
        name: string,
        value?: string,
        label?: string,
        autocomplete?: FullAutoFill | null | undefined,
        placeholder: string,
        title: string,
        disabled?: boolean | null | undefined,
        maxLength?: number | null | undefined,
        min?: string,
        max?: string,
        required?: boolean,
        pattern?: string,
        width?: string,

        showCopy?: boolean,
    } = $props();

    let isErr = $state(false);

    function copy() {
        if (navigator.clipboard) {
            navigator.clipboard.writeText(value);
        } else {
            console.error("Copy to clipboard is only available in secure contexts");
        }
    }

    function toggleView() {
        if (type === 'password') {
            type = 'text';
        } else {
            type = 'password';
        }
    }

    function onblur(event: FocusEvent & { currentTarget: EventTarget & HTMLInputElement }) {
        // console.log('in onBlur');color: var(--col-err);
        // console.log(event);
        const isValid = event?.currentTarget?.reportValidity();
        isErr = !isValid;
    }

    function oninput(event: Event & { currentTarget: EventTarget & HTMLInputElement }) {
        // console.log('in onInput');
        // console.log(event);
    }

    function oninvalid(event: Event & { currentTarget: EventTarget & HTMLInputElement }) {
        // console.log('in onInvalid');
        // console.log(event);
        event.preventDefault();
        isErr = true;
    }

    function onkeydown(ev: KeyboardEvent) {
        if (ev.code === 'Enter') {
            // TODO try to find out if we are in a form and submit it
            // dispatch('enter', true);
        }
    }

</script>

<div style:width={width}>
    <div class="input-row">
        <input
                {type}
                {id}
                {name}
                {title}
                aria-label={title}
                style:padding-right={showCopy ? '55px' : '30px'}
                bind:value

                {autocomplete}
                {placeholder}
                {disabled}

                required={required || undefined}
                maxlength={maxLength || undefined}
                min={min || undefined}
                max={max || undefined}
                pattern={pattern || undefined}

                {oninput}
                {oninvalid}
                {onblur}
                {onkeydown}
        />

        <div class="rel">
            {#if showCopy}
                <div
                        role="button"
                        tabindex="0"
                        class="btn clip"
                        onclick={copy}
                        onkeydown={copy}
                >
                    <IconClipboard/>
                </div>
            {/if}

            <div
                    role="button"
                    tabindex="0"
                    class="btn show"
                    onclick={toggleView}
                    onkeydown={toggleView}
            >
                {#if type === 'password'}
                    <IconEyeSlash width={22}/>
                {:else}
                    <IconEye width={22}/>
                {/if}
            </div>
        </div>
    </div>
</div>

<div class="label">
    <label for={id} class="font-label noselect" data-required={required}>
        {label}
    </label>
    {#if isErr}
        <div class="error" transition:slide>
            {#if !label}
                <div class="nolabel"></div>
            {/if}
            {title}
        </div>
    {/if}
</div>

<style>
    .error {
        margin-top: -.5rem;
        color: hsl(var(--error));
    }

    .input-row {
        display: flex;
        flex-direction: row;
    }

    label, .error {
        line-height: 1.1rem;
        font-size: .9rem;
    }

    label {
        flex-wrap: wrap;
    }

    .label {
        width: 100%;
        margin-top: -1.1rem;
        padding: .5rem;
    }

    .nolabel {
        height: .8rem;
    }

    .btn {
        position: absolute;
        top: 10px;
        right: 5px;
        margin-left: 100px;
        opacity: 0.85;
        cursor: pointer;
    }

    .clip {
        right: 32px;
        opacity: 0.85;
    }

    .show {
        opacity: 0.85;
    }

    .rel {
        position: relative;
        /*border: 1px solid green;*/
    }
</style>
