<script lang="ts">
    import Form from "$lib/components/form/Form.svelte";
    import Button from "$lib/components/Button.svelte";
    import InputPassword from "$lib/components/form/InputPassword.svelte";
    import {API_PREFIX, fetchGet} from "$lib/utils/fetch";
    import {storeSession} from "$lib/stores/session";
    // import {pow_work_wasm} from "../../spow/spow-wasm";

    const action = `${API_PREFIX}/session`;

    let error = $state('');
    let isLoading = $state(false);

    async function onSubmit(form: FormData, params: URLSearchParams) {
        error = '';
        isLoading = true;

        // TODO the PoW WASM produces an error when built into html right now:
        // Node.appendChild: Cannot add children to a Comment
        // As soon as we get rid of the `pow_work_wasm` import, the error goes away.
        // Probably a bug with the svelte 5 preview release right now.
        // -> check in the future if it's resolved and use a PoW again

        // let resPow = await fetchGet('/pow');
        // if (resPow.status !== 200) {
        //     let resp = await resPow.json();
        //     error = Object.values(resp)[0] as string;
        //     isLoading = false;
        //     return;
        // }
        //
        // let challenge = await resPow.text();
        // console.log('pow challenge: ' + challenge);
        // let pow = await pow_work_wasm(challenge);
        //
        // if (!pow) {
        //     error = 'Error calculating pow';
        //     isLoading = false;
        //     return;
        // }
        params.append('pow', "NoPowUntilSvelte5ErrorFixed");

        const res = await fetch(action, {
            method: 'POST',
            headers: {
                'Content-type': 'application/x-www-form-urlencoded',
            },
            body: params,
        });

        let resp = await res.json();
        if (res.status === 200) {
            storeSession.set(resp);
        } else {
            error = Object.values(resp)[0] as string;
        }

        isLoading = false;
    }

    // async function onResponse(res: Response) {
    //     let resp = await res.json();
    //     if (res.status === 200) {
    //         storeSession.set(resp);
    //     } else {
    //         error = Object.values(resp)[0] as string;
    //     }
    // }
</script>

<svelte:head>
    <meta property="description" content="Hiqlite Login"/>
    <title>Login</title>
</svelte:head>

<div class="container">
    <div class="login">
        <Form action={action} {onSubmit}>
            <!--        <Form action={action} onResponse={onResponse}>-->
            <InputPassword
                    id="password"
                    name="password"
                    autocomplete="current-password"
                    placeholder="Password"
                    title="Valid Dashboard Password"
                    required
            />
            <Button type="submit" level={1} {isLoading}>
                Login
            </Button>

            {#if error}
                <div class="err">
                    {error}
                </div>
            {/if}
        </Form>
    </div>
</div>

<style>
    .container {
        width: 100dvw;
        flex: 1;
        display: flex;
        justify-content: center;
        align-items: center;
    }

    .login {
        max-width: 15rem;
        display: flex;
        height: 100dvh;
        justify-content: center;
        align-items: center;
    }
</style>
