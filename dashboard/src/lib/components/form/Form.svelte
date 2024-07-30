<script lang="ts">
    import {handleRes} from "$lib/fetch";

    let {
        action,
        method = 'POST',
        isError = $bindable(),
        onSubmit,
        onResponse,
    }: {
        action: string,
        method?: string,
        isError?: boolean,
        onSubmit?: (form: HTMLFormElement, params: URLSearchParams) => void,
        onResponse?: (res: Response) => void,
    } = $props();

    async function submit(event: SubmitEvent & { currentTarget: EventTarget & HTMLFormElement }) {
        event.preventDefault();
        const form = event.currentTarget;

        const isValid = form.reportValidity();
        if (isValid) {
            isError = false;
        } else {
            isError = true;
            return;
        }

        const formData = new FormData(form);
        let params = new URLSearchParams();
        formData.forEach((value, key) => {
            params.append(key, value.toString());
        })

        if (onSubmit) {
            onSubmit(form, params);
            return;
        }

        const res = await fetch(form.action, {
            method: form.method,
            headers: {
                'Content-type': 'application/x-www-form-urlencoded',
            },
            body: params,
        });
        // always check for 401
        handleRes(res);

        if (onResponse) {
            onResponse(res);
            if (res.ok) {
                form.reset();
            }
        }
    }
</script>

<form action={action} method={method} onsubmit={submit}>
    <slot/>
</form>
