<script lang="ts">
	import { Pagination as PaginationPrimitive } from "bits-ui";
	import CaretLeft from "phosphor-svelte/lib/CaretLeft";
	import { buttonVariants } from "$lib/components/ui/button/index.js";
	import { cn } from "$lib/components/ai-elements/markdown/utils/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: PaginationPrimitive.PrevButtonProps = $props();
</script>

{#snippet Fallback()}
	<CaretLeft class="size-4" />
	<span>Previous</span>
{/snippet}

<PaginationPrimitive.PrevButton
	bind:ref
	aria-label="Go to previous page"
	class={cn(
		buttonVariants({
			size: "default",
			variant: "ghost",
			class: "gap-1 px-2.5 sm:ps-2.5",
		}),
		className
	)}
	children={children || Fallback}
	{...restProps}
/>
