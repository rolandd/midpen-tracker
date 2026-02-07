## 2026-05-20 - Skip to Content Links
**Learning:** Sticky headers and complex navigation create significant barriers for keyboard users, who must tab through every menu item on every page load.
**Action:** Always include a "Skip to Content" link as the first focusable element in the layout. Use `sr-only focus:not-sr-only` utility classes to hide it visually until focused, and ensure the target element (e.g., `<main>`) has `id="main-content"` and `tabindex="-1"` to receive focus programmatically.
