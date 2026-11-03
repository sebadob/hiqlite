import {type Writable, writable} from 'svelte/store';
import type {ISession} from "#lib/types/session.js";

export const storeSession: Writable<undefined | ISession> = writable(undefined);
