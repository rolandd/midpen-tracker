<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		description?: string;
		icon?: string;
		variant?: 'default' | 'compact';
		action?: Snippet;
	}

	let { title, description, icon = '📭', variant = 'default', action }: Props = $props();

	const variantClasses = {
		default: 'p-12',
		compact: 'p-6'
	};
</script>

<div class="empty-state {variantClasses[variant]}">
	<div class="icon" aria-hidden="true" class:compact={variant === 'compact'}>{icon}</div>
	<h3 class="title" class:compact={variant === 'compact'}>{title}</h3>
	{#if description}
		<p class="description" class:compact={variant === 'compact'}>{description}</p>
	{/if}
	{#if action}
		<div class="action" class:compact={variant === 'compact'}>
			{@render action()}
		</div>
	{/if}
</div>

<style>
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		background: var(--color-surface);
		border: 1px dashed var(--color-border);
		border-radius: var(--radius);
		color: var(--color-text-muted);
	}

	.icon {
		font-size: 3rem;
		margin-bottom: 1rem;
		opacity: 0.5;
	}

	.icon.compact {
		font-size: 2rem;
		margin-bottom: 0.5rem;
	}

	.title {
		font-size: 1.125rem;
		font-weight: 600;
		color: var(--color-text);
		margin-bottom: 0.5rem;
	}

	.title.compact {
		font-size: 1rem;
		margin-bottom: 0.25rem;
	}

	.description {
		max-width: 24rem;
		margin-bottom: 0;
		font-size: 0.875rem;
		line-height: 1.5;
	}

	.action {
		margin-top: 1.5rem;
	}

	.action.compact {
		margin-top: 1rem;
	}
</style>
