<script lang="ts">
    import {untrack} from "svelte";

    import type {Snippet} from "svelte";

    let {
        resizeRight,
        resizeBottom,
        border,
        padding,
        initialWidthPx,
        initialHeightPx,
        minWidthPx = 50,
        minHeightPx = 50,
        children,
    }: {
        resizeRight?: boolean,
        resizeBottom?: boolean,
        border?: string,
        padding?: string,
        initialWidthPx?: number,
        initialHeightPx?: number,
        minWidthPx?: number,
        minHeightPx?: number,
        children: Snippet,
    } = $props();

    let ref: undefined | HTMLDivElement;

    let top: undefined | number = $state();
    let left: undefined | number = $state();
    let width: undefined | number = $state(untrack(() => initialWidthPx));
    let height: undefined | number = $state(untrack(() => initialHeightPx));

    $effect(() => {
        updateRef();
    });

    function updateRef() {
        if (ref) {
            let rect = ref.getBoundingClientRect();

            if (resizeRight) {
                left = rect.left;
                width = rect.width;
            }

            if (resizeBottom) {
                top = rect.top;
                height = rect.height
            }
        }
    }

    function onMouseDownRight() {
        updateRef();
        window.addEventListener('mousemove', onMoveRight);
        window.addEventListener('mouseup', onMouseUpRight, {once: true});
    }

    function onMouseUpRight() {
        window.removeEventListener('mousemove', onMoveRight);
        updateRef();
    }

    function onMoveRight(ev: MouseEvent) {
        let x = window.scrollX + ev.x - (left || 0);
        if (x < minWidthPx) {
            width = minWidthPx;
        } else {
            width = x;
        }
    }

    function onMouseDownBottom() {
        updateRef();
        window.addEventListener('mousemove', onMoveBottom);
        window.addEventListener('mouseup', onMouseUpBottom, {once: true});
    }

    function onMouseUpBottom() {
        window.removeEventListener('mousemove', onMoveBottom);
        updateRef();
    }

    function onMoveBottom(ev: MouseEvent) {
        let y = window.screenY + ev.y - (top || 0);
        if (y < minHeightPx) {
            height = minHeightPx;
        } else {
            height = y;
        }
    }
</script>

<div
        bind:this={ref}
        style:width={width && `${width}px`}
        style:height={height && `${height}px`}
        style:border
        style:padding
>
    <div class="children">
        <div class="inner">
            {@render children()}
        </div>

        {#if resizeRight}
            <div class="relative">
                <div
                        role="none"
                        class="right"
                        onmousedown={onMouseDownRight}
                >
                </div>
            </div>
        {/if}
    </div>

    {#if resizeBottom}
        <div class="relative">
            <div role="none" class="bottom" onmousedown={onMouseDownBottom}>
            </div>
        </div>
    {/if}
</div>

<style>
    .children {
        display: flex;
        height: 100%;
    }

    .inner {
        height: 100%;
        flex: 1;
        overflow: auto;
    }

    .right, .bottom {
        position: absolute;
        transition: all 150ms;
    }

    .right {
        height: 100%;
        width: 12px;
        top: 0;
        right: -6px;
    }

    .bottom {
        left: 0;
        height: 12px;
        bottom: -6px;
    }

    .bottom {
        left: 0;
        width: 100%;
    }

    .right:hover,
    .bottom:hover {
        background: hsla(var(--bg-high), .66);
    }

    .right:hover {
        cursor: col-resize;
    }

    .bottom:hover {
        cursor: row-resize;
    }
</style>