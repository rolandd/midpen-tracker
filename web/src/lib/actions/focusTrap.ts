export function focusTrap(node: HTMLElement) {
	const previousActiveElement = document.activeElement as HTMLElement;

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Tab') {
			const focusable = node.querySelectorAll<HTMLElement>(
				'button:not([disabled]), [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
			);

			if (!focusable || focusable.length === 0) {
				e.preventDefault();
				return;
			}

			const first = focusable[0];
			const last = focusable[focusable.length - 1];

			if (e.shiftKey) {
				if (document.activeElement === first) {
					e.preventDefault();
					last.focus();
				}
			} else {
				if (document.activeElement === last) {
					e.preventDefault();
					first.focus();
				}
			}
		}
	}

	node.addEventListener('keydown', handleKeydown);

	// Initial focus
	const firstFocusable = node.querySelector<HTMLElement>(
		'button:not([disabled]), [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
	);
	if (firstFocusable) {
		firstFocusable.focus();
	} else {
		// Fallback to container if no focusable elements found
		node.focus();
	}

	return {
		destroy() {
			node.removeEventListener('keydown', handleKeydown);
			previousActiveElement?.focus();
		}
	};
}
