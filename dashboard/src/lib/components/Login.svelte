<script lang="ts">
    import Form from "$lib/components/form/Form.svelte";
    import Button from "$lib/components/Button.svelte";
    import InputPassword from "$lib/components/form/InputPassword.svelte";
    import {API_PREFIX} from "$lib/utils/fetch";
    import {storeSession} from "$lib/stores/session";

    async function onResponse(res: Response) {
        if (res.status === 200) {
            let s = await res.json();
            storeSession.set(s);
        }
    }
</script>

<svelte:head>
    <meta property="description" content="Hiqlite Login"/>
    <title>Login</title>
</svelte:head>

<div class="container">
    <div class="login">
        <Form action={`${API_PREFIX}/session`} onResponse={onResponse}>
            <InputPassword
                    id="password"
                    name="password"
                    autocomplete="current-password"
                    placeholder="Password"
                    title="Valid Dashboard Password"
                    required
            />
            <Button type="submit">
                Login
            </Button>
        </Form>
    </div>
</div>

<style>
    .container {
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
