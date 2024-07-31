<script lang="ts">
    import ThemeSwitchAbsolute from "$lib/components/ThemeSwitchAbsolute.svelte";
    import {onMount} from "svelte";
    import type {ISession} from "$lib/types/session";
    import Login from "$lib/components/Login.svelte";
    import Tables from "$lib/components/tables/Tables.svelte";
    import {storeSession} from "$lib/stores/session";
    import Health from "$lib/components/health/Health.svelte";
    import {API_PREFIX} from "$lib/utils/fetch";

    let session: undefined | ISession = $state();

    storeSession.subscribe(s => {
        session = s;
    })

    $effect(() => {
        console.log(session);
    });

    onMount(async () => {
        let res = await fetch(`${API_PREFIX}/session`);
        if (res.status === 200) {
            let s = await res.json();
            console.log(s);
            storeSession.set(s);
        }

        // if (res.status === 401) {
        //     mustLogin = true;
        // } else {
        //     let s = await res.json();
        //     console.log(s);
        //     storeSession.set(s);
        // }
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
{:else}
    <Login bind:session/>
    <!--{:else if mustLogin}-->
    <!--    <Login bind:session/>-->
    <!--{:else}-->
    <!--    <main>-->
    <!--        <Loading/>-->
    <!--    </main>-->
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
