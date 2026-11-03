import type {SqlValue} from '#lib/types/query_results.js';

/// Renders a `SqlValue` for a table cell, keeping a SQL NULL distinguishable from
/// an empty string.
export function sqlValueDisplay(value: SqlValue): string | number {
    if (value === 'Null') {
        return 'NULL';
    }
    if ('Integer' in value) {
        return value.Integer;
    }
    if ('Real' in value) {
        return value.Real;
    }
    if ('Text' in value) {
        return value.Text;
    }
    return `x'${buf2hex(value.Blob)}'`;
}

export function isNumericSqlValue(value: SqlValue): boolean {
    return value !== 'Null' && ('Integer' in value || 'Real' in value);
}

function buf2hex(bytes: number[]) {
    return bytes.map(x => x.toString(16).padStart(2, '0')).join('');
}
