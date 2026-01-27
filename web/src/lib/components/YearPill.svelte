<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	interface Props {
		active?: boolean;
		onclick?: () => void;
		label: string;
	}

	let { active = false, onclick, label }: Props = $props();
</script>

<button
	class="px-4 py-2 rounded-full text-sm font-medium transition-all duration-200 border
		{active
		? 'bg-primary border-primary text-white'
		: 'bg-surface border-border text-text-muted hover:border-primary/50 hover:text-text'}"
	{onclick}
>
	{label}
</button>
