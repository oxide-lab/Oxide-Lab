<script lang="ts">
	import { Pagination as PaginationPrimitive } from "bits-ui";
	import CaretRight from "phosphor-svelte/lib/CaretRight";
	import { buttonVariants } from "$lib/components/ui/button/index.js";
	import { cn } from "$lib/components/ai-elements/markdown/utils/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: PaginationPrimitive.NextButtonProps = $props();
</script>

{#snippet Fallback()}
	<span>Next</span>
	<CaretRight class="size-4" />
{/snippet}

<PaginationPrimitive.NextButton
	bind:ref
	aria-label="Go to next page"
	class={cn(
		buttonVariants({
			size: "default",
			variant: "ghost",
			class: "gap-1 px-2.5 sm:pe-2.5",
		}),
		className
	)}
	children={children || Fallback}
	{...restProps}
/>
