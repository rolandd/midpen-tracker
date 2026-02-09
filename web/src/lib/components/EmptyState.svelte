<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		description?: string;
		icon?: Snippet;
		action?: Snippet;
		variant?: 'default' | 'compact';
	}

	let { title, description, icon, action, variant = 'default' }: Props = $props();

	const variantClasses = {
		default: 'py-16 px-4',
		compact: 'py-8 px-4 text-sm'
	};
</script>

<div
	class="flex flex-col items-center justify-center text-center rounded-[var(--radius)] border border-dashed border-[var(--color-border)] bg-[var(--color-surface)]/50 {variantClasses[
		variant
	]}"
>
	<div class="mb-4 text-[var(--color-text-muted)] opacity-50">
		{#if icon}
			{@render icon()}
		{:else}
			<!-- Default ghost icon -->
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="48"
				height="48"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
				stroke-linejoin="round"
				class={variant === 'compact' ? 'w-8 h-8' : 'w-12 h-12'}
			>
				<path d="M9 10h.01" />
				<path d="M15 10h.01" />
				<path d="M12 2a8 8 0 0 0-8 8v12l3-3 2.5 2.5L12 19l2.5 2.5L17 19l3 3V10a8 8 0 0 0-8-8z" />
			</svg>
		{/if}
	</div>

	<h3
		class="font-semibold text-[var(--color-text)] mb-2 {variant === 'compact'
			? 'text-base'
			: 'text-lg'}"
	>
		{title}
	</h3>

	{#if description}
		<p
			class="text-[var(--color-text-muted)] max-w-sm mb-6 leading-relaxed {variant === 'compact'
				? 'text-xs mb-3'
				: 'text-sm'}"
		>
			{description}
		</p>
	{/if}

	{#if action}
		<div class="mt-2">
			{@render action()}
		</div>
	{/if}
</div>
