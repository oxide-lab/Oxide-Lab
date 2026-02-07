<script lang="ts">
	import type { HTMLLiAttributes } from "svelte/elements";
	import type { WithElementRef } from "$lib/components/ai-elements/markdown/utils/utils.js";

	let {
		ref = $bindable(null),
		children,
		...restProps
	}: WithElementRef<HTMLLiAttributes> = $props();
</script>

<li bind:this={ref} data-slot="pagination-item" {...restProps}>
	{@render children?.()}
</li>
