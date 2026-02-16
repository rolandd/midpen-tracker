## 2025-05-23 - Accessibility of Accordion Patterns
**Learning:** Accordion toggles implemented with `div`s and `role="button"` are often missing the `aria-expanded` state, making it impossible for screen reader users to know if the section is open or closed.
**Action:** Always verify `aria-expanded` exists on any element that toggles visibility of another element.

## 2026-01-24 - Accessibility of New Tab Links
**Learning:** Links opening in a new tab (`target="_blank"`) without warning can disorient screen reader users. The content of activity links (emoji, date, arrow) was disjointed.
**Action:** Use `aria-label` to provide a cohesive description and explicitly state "opens in new tab" for such links.

## 2026-02-01 - Standardizing Destructive Actions
**Learning:** Destructive actions (like delete) were implemented with one-off CSS and markup, leading to inconsistent focus states and loading feedback.
**Action:** Extended the design system's `Button` component with a `danger` variant to ensure consistent visual language and accessibility (keyboard support, loading states) for all destructive actions.

## 2026-02-03 - Accessibility of Filter Groups
**Learning:** Filter sets implemented as buttons often lack state information. A group of "radio-style" buttons needs `aria-pressed` (or `aria-current`) to indicate which filter is currently active.
**Action:** Use `role="group"` on the container and `aria-pressed` on the active button for filter/toggle sets.

## 2025-10-26 - Standardized Loading States
**Learning:** Replacing ad-hoc loading spinners with a standardized `Button` component (with `isLoading` prop) significantly improves UX by maintaining focus context and preventing layout shifts.
**Action:** Always check if `Button` component supports `isLoading` before manually adding spinners next to buttons.

## 2026-06-03 - Empty States with Actions
**Learning:** Empty states are often dead ends for users. Providing a direct action (e.g., "Show unvisited") within the empty state transforms a negative experience into a helpful navigation aid.
**Action:** When implementing empty states, always consider if there is a primary action the user should take next, and include it via the `action` snippet prop in the `EmptyState` component.

## 2025-05-21 - Modal Focus Trap & Backdrop Pattern
**Learning:** Svelte's a11y checks flag interactive backdrops (click-to-dismiss) if they lack keyboard handlers, even if a global `window` handler manages the `Escape` key.
**Action:** When implementing custom modals:
1. Use `e.target === e.currentTarget` on the backdrop `onclick` to avoid needing `stopPropagation` on the modal content.
2. Rely on `svelte:window` for global `Escape` handling.
3. Explicitly suppress `a11y_click_events_have_key_events` on the backdrop if the keyboard interaction is handled globally, or add a dummy handler if preferred.
4. Always implement a focus trap using `bind:this` and `keydown` interception for Tab/Shift+Tab.

## 2026-03-05 - Reusable Focus Trap Action
**Learning:** Encapsulating complex accessibility behaviors like focus trapping into a reusable Svelte action keeps components declarative and ensures consistent behavior across the application.
**Action:** When encountering repeated DOM manipulation patterns for accessibility (like focus management or click-outside), extract them into actions in `web/src/lib/actions/`.
