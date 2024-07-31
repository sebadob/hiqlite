import {storeSession} from "$lib/stores/session";

// const HEADERS = {
//     'Content-Type': 'application/json',
//     'Accept': 'application/json',
// }

export const API_PREFIX = '/dashboard/api';

export async function fetchGet(url: string) {
    let res = await fetch(`${API_PREFIX}${url}`, {
        method: 'GET',
        // headers: HEADERS,
    });
    return handleRes(res);
}

export async function fetchPost(url: string, payload: any) {
    let res = await fetch(`${API_PREFIX}${url}`, {
        method: 'POST',
        // headers: HEADERS,
        body: JSON.stringify(payload),
    });
    return handleRes(res);
}

export async function fetchPostText(url: string, payload: any) {
    let res = await fetch(`${API_PREFIX}${url}`, {
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