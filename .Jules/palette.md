## 2025-05-21 - Modal Focus Trap & Backdrop Pattern
**Learning:** Svelte's a11y checks flag interactive backdrops (click-to-dismiss) if they lack keyboard handlers, even if a global `window` handler manages the `Escape` key.
**Action:** When implementing custom modals:
1. Use `e.target === e.currentTarget` on the backdrop `onclick` to avoid needing `stopPropagation` on the modal content.
2. Rely on `svelte:window` for global `Escape` handling.
3. Explicitly suppress `a11y_click_events_have_key_events` on the backdrop if the keyboard interaction is handled globally, or add a dummy handler if preferred.
4. Always implement a focus trap using `bind:this` and `keydown` interception for Tab/Shift+Tab.
