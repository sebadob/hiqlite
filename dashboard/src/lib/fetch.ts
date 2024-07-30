import {storeSession} from "$lib/stores/session";

// const HEADERS = {
//     'Content-Type': 'application/json',
//     'Accept': 'application/json',
// }

export async function get(url: string) {
    let res = await fetch(url, {
        method: 'GET',
        // headers: HEADERS,
    });
    return handleRes(res);
}

export async function post(url: string, payload: any) {
    let res = await fetch(url, {
        method: 'POST',
        // headers: HEADERS,
        body: JSON.stringify(payload),
    });
    return handleRes(res);
}

export async function postText(url: string, payload: any) {
    let res = await fetch(url, {
        method: 'POST',
        // headers: HEADERS,
        body: payload,
    });
    return handleRes(res);
}

export function handleRes(res: Response) {
    if (res.status === 401) {
        storeSession.set(undefined);
    }
    return res;
}