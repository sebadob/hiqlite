import type {IQuery} from "$lib/types/query";

export const DEFAULT_QUERY = '-- comments will be ignored but only a single query is allowed\n' +
    '-- press CTRL + Enter to execute\n' +
    'SELECT 1';

export const DEFAULT_QUERY_FULL: IQuery = {
    id: 'SELECT 1',
    query: DEFAULT_QUERY,
}

export const AUTO_QUERY = '--!auto-query';

export let QUERIES = $state([DEFAULT_QUERY_FULL]);
