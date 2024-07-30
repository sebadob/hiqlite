<script lang="ts">
    import ThemeSwitchAbsolute from "$lib/components/ThemeSwitchAbsolute.svelte";
    import {onMount} from "svelte";
    import Loading from "$lib/components/Loading.svelte";
    import type {ISession} from "$lib/types/session";
    import {API_PREFIX} from "$lib/constants";
    import Login from "$lib/components/Login.svelte";
    import Tables from "$lib/components/tables/Tables.svelte";
    import {storeSession} from "$lib/stores/session";
    import Health from "$lib/components/health/Health.svelte";

    let session: undefined | ISession = $state();
    let mustLogin = $state(false);

    storeSession.subscribe(s => {
        session = s;
    })

    onMount(async () => {
        let res = await fetch(`${API_PREFIX}/session`);
        if (res.status === 401) {
            mustLogin = true;
        } else {
            let s = await res.json();
            console.log(s);
            storeSession.set(s);
        }
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
{:else if mustLogin}
    <Login bind:session/>
{:else}
    <main>
        <Loading/>
    </main>
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
        height: 100dvh;
        width: 100dvw;
        display: flex;
        flex-direction: column;
        align-items: center;
    }
</style>
