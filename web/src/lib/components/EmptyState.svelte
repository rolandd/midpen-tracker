<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title?: string;
		description: string;
		icon?: string;
		variant?: 'default' | 'compact';
		action?: Snippet;
	}

	let { title, description, icon, variant = 'default', action }: Props = $props();
</script>

<div class="empty-state {variant}">
	{#if icon}
		<div class="icon" aria-hidden="true">{icon}</div>
	{/if}
	{#if title}
		<h3>{title}</h3>
	{/if}
	<p>{description}</p>
	{#if action}
		<div class="action">
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
		color: var(--color-text-muted);
	}

	.empty-state.default {
		padding: 3rem 1.5rem;
		background: var(--color-surface);
		border: 1px dashed var(--color-border);
		border-radius: var(--radius, 12px);
		min-height: 200px;
	}

	.empty-state.compact {
		padding: 1rem;
		background: transparent;
		border: none;
		font-size: 0.875rem;
		font-style: italic;
	}

	.icon {
		font-size: 2.5rem;
		margin-bottom: 1rem;
		opacity: 0.5;
		line-height: 1;
	}

	.compact .icon {
		font-size: 1.5rem;
		margin-bottom: 0.5rem;
	}

	h3 {
		font-size: 1.125rem;
		font-weight: 600;
		color: var(--color-text);
		margin: 0 0 0.5rem 0;
	}

	.compact h3 {
		font-size: 1rem;
	}

	p {
		margin: 0;
		max-width: 24rem;
		line-height: 1.5;
	}

	.action {
		margin-top: 1.5rem;
	}

	.compact .action {
		margin-top: 0.75rem;
	}
</style>
