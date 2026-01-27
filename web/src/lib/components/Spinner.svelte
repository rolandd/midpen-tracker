<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	interface Props {
		size?: 'sm' | 'md' | 'lg';
		variant?: 'primary' | 'white';
	}

	let { size = 'md', variant = 'primary' }: Props = $props();

	const sizeClasses = {
		sm: 'w-5 h-5 border-2',
		md: 'w-8 h-8 border-3',
		lg: 'w-12 h-12 border-3'
	};

	const variantClasses = {
		primary: 'border-border border-t-primary',
		white: 'border-white/30 border-t-white'
	};
</script>

<div
	class="{sizeClasses[size]} {variantClasses[variant]} rounded-full animate-spin"
	role="status"
	aria-label="Loading"
></div>

<style>
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
	.animate-spin {
		animation: spin 1s linear infinite;
	}
</style>
