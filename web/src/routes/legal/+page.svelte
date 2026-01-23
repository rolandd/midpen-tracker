<script lang="ts">
	// @ts-ignore
	import Legal from '../../lib/assets/legal.md';
</script>

<div class="legal-page">
	<div class="content">
		<Legal />
	</div>
</div>

<style>
	.legal-page {
		max-width: 800px;
		margin: 0 auto;
		padding: 4rem 1.5rem;
		line-height: 1.6;
	}

	.content {
		background: rgba(255, 255, 255, 0.02);
		padding: 2.5rem;
		border-radius: var(--radius-lg);
		border: 1px solid var(--color-border);
	}

	/* Use :global to ensure styles apply to markdown-generated HTML */
	
	:global(.legal-page h1),
	:global(.legal-page h2),
	:global(.legal-page h3) {
		color: #fff !important;
		margin-top: 2rem;
		margin-bottom: 1rem;
	}

	:global(.legal-page h1) {
		font-size: 2.5rem;
		background: linear-gradient(135deg, #fff 0%, #8b95a7 100%);
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}

	:global(.legal-page h2) {
		font-size: 1.5rem;
	}

	/* Top level content should be bright white */
	:global(.legal-page p),
	:global(.legal-page ol > li),
	:global(.legal-page ul > li) {
		color: #fff !important;
	}

	/* Nested list items and everything inside them should be gray */
	:global(.legal-page li li),
	:global(.legal-page li li *),
	:global(.legal-page li p),
	:global(.legal-page li ul),
	:global(.legal-page li ol) {
		color: var(--color-text-muted) !important;
	}

	/* Ensure list markers inherit the color of their list item */
	:global(.legal-page li::marker) {
		color: inherit !important;
	}

	/* List resets/spacing */
	:global(.legal-page ul) {
		list-style-type: disc;
		margin-left: 1.5rem;
		margin-bottom: 1rem;
	}

	:global(.legal-page ol) {
		list-style-type: decimal;
		margin-left: 1.5rem;
		margin-bottom: 1rem;
	}

	:global(.legal-page li) {
		margin-bottom: 0.5rem;
	}
</style>
