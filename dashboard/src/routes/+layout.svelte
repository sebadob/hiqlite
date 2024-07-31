<script lang="ts">
    import ThemeSwitchAbsolute from "$lib/components/ThemeSwitchAbsolute.svelte";
    import {onMount} from "svelte";
    import type {ISession} from "$lib/types/session";
    import Login from "$lib/components/Login.svelte";
    import Tables from "$lib/components/tables/Tables.svelte";
    import {storeSession} from "$lib/stores/session";
    import Health from "$lib/components/health/Health.svelte";
    import {API_PREFIX} from "$lib/utils/fetch";
    import {useSignal} from "$lib/stores/sharedRune.svelte";
    import {DEFAULT_QUERY_FULL} from "$lib/stores/query.svelte.js";

    let session: undefined | ISession = $state();
    let isInitialized = $state(false);

    let queries = useSignal('queries', [DEFAULT_QUERY_FULL]);

    storeSession.subscribe(s => {
        session = s;
    })

    $effect(() => {
        console.log(session);
    });

    onMount(async () => {
        let res = await fetch(`${API_PREFIX}/session`);
        if (res.status === 200) {
            storeSession.set(await res.json());
        }
        isInitialized = true;
    });

</script>

<svelte:head>
    <meta name="robots" content="noindex nofollow"/>
</svelte:head>

{#if session}
    <nav>
        <Tables/>
    </nav>
    <main>
        <slot/>
    </main>
    <Health/>
{:else if isInitialized}
    <Login/>
{/if}

<ThemeSwitchAbsolute/>

<style>
    nav {
        height: 100dvh;
        display: flex;
        flex-direction: column;
        border-right: 1px solid #808080;
    }

    main {
        flex: 1;
        display: flex;
        flex-direction: column;
    }
</style>
