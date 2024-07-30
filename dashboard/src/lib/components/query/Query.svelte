<script lang="ts">
    import {API_PREFIX} from "$lib/constants";
    import {postText} from "$lib/fetch";

    let {query}: { query: string } = $props();

    function onKeyDown(ev: KeyboardEvent) {
        if (ev.ctrlKey && ev.code === 'Enter') {
            console.log('ctrl + enter');
            execute();
        }
    }

    async function execute() {
        let res = await postText(`${API_PREFIX}/query`, query);
        console.log(res);
    }
</script>

<textarea
        name="query"
        bind:value={query}
        onkeydown={onKeyDown}
>
</textarea>

<style>
    textarea {
        width: calc(100% - 20px);
        height: 20rem;
        padding: 10px;
        border: 1px solid var(--col-gmid);
        border-radius: 3px;
        outline: none;
        resize: none;
        font-size: 1.1rem;
        background: var(--col-bg);
        border-bottom: 1px solid var(--col-mid-a);
    }

    textarea:focus {
        resize: none;
    }
</style>