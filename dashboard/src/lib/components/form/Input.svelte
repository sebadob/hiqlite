<script lang="ts">
    import {slide} from "svelte/transition";

    let {
        type = 'text',
        id,
        name = '',
        value,
        label,
        autocomplete = 'on',
        placeholder = '',
        title = '',
        disabled = false,
        maxLength,
        min,
        max,
        required = false,
        pattern,
        width = 'inherit',
    }: {
        type?: string,
        id?: undefined | string,
        name: string,
        value?: string,
        label?: string,
        autocomplete?: string,
        placeholder: string,
        title: string,
        disabled?: boolean | null | undefined,
        maxLength?: number | null | undefined,
        min?: string,
        max?: string,
        required?: boolean,
        pattern?: string,
        width?: string,
    } = $props();

    let isErr = $state(false);

    function handleKeyPress(ev: KeyboardEvent) {
        if (ev.code === 'Enter') {
            // TODO try to find out if we are in a form and submit it
            // dispatch('enter', true);
        }
    }

    function onBlur(event: FocusEvent & { currentTarget: EventTarget & HTMLInputElement }) {
        // console.log('in onBlur');
        // console.log(event);
        const isValid = event?.currentTarget?.reportValidity();
        isErr = !isValid;
    }

    function onInput(event: Event & { currentTarget: EventTarget & HTMLInputElement }) {
        // console.log('in onInput');
        // console.log(event);
    }

    function onInvalid(event: Event & { currentTarget: EventTarget & HTMLInputElement }) {
        // console.log('in onInvalid');
        // console.log(event);
        event.preventDefault();
        isErr = true;
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
                bind:value

                {autocomplete}
                {placeholder}
                {disabled}

                required={required || undefined}
                maxlength={maxLength || undefined}
                min={min || undefined}
                max={max || undefined}
                pattern={pattern || undefined}

                oninput={onInput}
                oninvalid={onInvalid}
                onblur={onBlur}
                onkeydown={handleKeyPress}
        />
    </div>
    <div class="label">
        <label for={id} class="font-label noselect">
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
</div>

<style>
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

    .error {
        margin-top: -.5rem;
        color: var(--col-err);
    }
</style>
