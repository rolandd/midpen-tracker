<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import type { Snippet } from 'svelte';
	import Spinner from './Spinner.svelte';

	interface Props {
		variant?: 'primary' | 'secondary' | 'ghost';
		size?: 'sm' | 'md' | 'lg';
		onclick?: () => void;
		disabled?: boolean;
		isLoading?: boolean;
		children: Snippet;
	}

	let {
		variant = 'primary',
		size = 'md',
		onclick,
		disabled = false,
		isLoading = false,
		children
	}: Props = $props();

	const baseClasses =
		'inline-flex items-center justify-center gap-2 font-medium rounded-full transition-all duration-200 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed active:scale-95';

	const sizeClasses = {
		sm: 'px-4 py-2 text-sm',
		md: 'px-6 py-2.5 text-sm',
		lg: 'px-8 py-3 text-base'
	};

	const variantClasses = {
		primary:
			'bg-primary text-white hover:bg-primary-hover hover:-translate-y-0.5 hover:shadow-lg hover:shadow-primary/30',
		secondary:
			'bg-surface text-text-muted border border-border hover:border-primary hover:text-text',
		ghost: 'bg-transparent text-text-muted hover:text-text hover:bg-surface'
	};
</script>

<button
	class="{baseClasses} {sizeClasses[size]} {variantClasses[variant]}"
	{onclick}
	disabled={disabled || isLoading}
>
	{#if isLoading}
		<Spinner size="sm" variant={variant === 'primary' ? 'white' : 'primary'} />
	{/if}
	{@render children()}
</button>
