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
		'inline-flex items-center justify-center gap-2 font-semibold rounded-lg transition-all duration-200 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed';

	const sizeClasses = {
		sm: 'px-3 py-1.5 text-sm',
		md: 'px-5 py-2.5 text-base',
		lg: 'px-6 py-3 text-lg'
	};

	const variantClasses = {
		primary:
			'bg-primary text-white hover:bg-primary-hover hover:-translate-y-0.5 hover:shadow-lg hover:shadow-primary/30',
		secondary:
			'bg-surface text-text border border-border hover:bg-surface-hover hover:border-primary/50',
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
