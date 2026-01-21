## 2025-05-23 - Accessibility of Accordion Patterns
**Learning:** Accordion toggles implemented with `div`s and `role="button"` are often missing the `aria-expanded` state, making it impossible for screen reader users to know if the section is open or closed.
**Action:** Always verify `aria-expanded` exists on any element that toggles visibility of another element.
