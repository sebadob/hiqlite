<script lang="ts">
    import type {Snippet} from 'svelte';
    import {page} from "$app/stores";

    let {
        href,
        target,
        selectedStep = false,
        hideUnderline = false,
        children,
    }: {
        href: string,
        target?: string,
        selectedStep?: boolean,
        hideUnderline?: boolean,
        children: Snippet,
    } = $props();

    let ariaCurrentType: 'page' | 'time' | 'step' | 'location' | 'date' | undefined = $derived.by(() => {
        if (selectedStep) {
            return 'step';
        }

        if ($page.route.id === href.split('?')[0]) {
            return 'page'
        }

        return undefined;
    });

</script>

<span class="font-label">
    <a
            class:hideUnderline
            {href}
            {target}
            aria-current={ariaCurrentType}
    >
        {@render children()}
    </a>
</span>

<style>
    a, a:link, a:visited {
        color: hsl(var(--text));
        transition: all 150ms ease-in-out;
    }

    .hideUnderline {
        text-decoration: none;
    }

    a:hover, a:active {
        color: hsl(var(--action));
        text-decoration: underline;
    }

    a[aria-current="page"],
    a[aria-current="step"] {
        text-decoration: underline;
    }
</style>
