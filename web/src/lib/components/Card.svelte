<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		padding?: 'sm' | 'md' | 'lg';
		hover?: boolean;
		children: Snippet;
	}

	let { padding = 'md', hover = false, children }: Props = $props();

	const paddingClasses = {
		sm: 'p-3',
		md: 'p-5',
		lg: 'p-6'
	};
</script>

<div
	class="bg-surface rounded-lg border border-border {paddingClasses[padding]} {hover
		? 'transition-colors hover:bg-surface-hover cursor-pointer'
		: ''}"
>
	{@render children()}
</div>
