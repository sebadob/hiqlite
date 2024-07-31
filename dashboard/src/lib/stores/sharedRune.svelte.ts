import {getContext, hasContext, setContext} from "svelte";

export const useSharedStore = <T, A>(
    name: string,
    fn: (value?: A) => T,
    defaultValue?: A,
) => {
    if (hasContext(name)) {
        return getContext<T>(name);
    }
    const _value = fn(defaultValue);
    setContext(name, _value);
    return _value;
};

// export const useWritable = <T>(name: string, value?: T) =>
//     useSharedStore(name, writable, value);
//
// export const useReadable = <T>(name: string, value: T) =>
//     useSharedStore(name, readable, value);

export const useSignal = <T>(name: string, value: T) =>
    useSharedStore(name, signal, value);


export const signal = <T>(initialValue: T) => {
    let signal = $state(initialValue);
    return {
        get value() {
            return signal;
        },
        set value(v: T) {
            signal = v;
        }
    };
};
