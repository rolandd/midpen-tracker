## 2025-05-23 - Accessibility of Accordion Patterns
**Learning:** Accordion toggles implemented with `div`s and `role="button"` are often missing the `aria-expanded` state, making it impossible for screen reader users to know if the section is open or closed.
**Action:** Always verify `aria-expanded` exists on any element that toggles visibility of another element.

## 2026-01-24 - Accessibility of New Tab Links
**Learning:** Links opening in a new tab (`target="_blank"`) without warning can disorient screen reader users. The content of activity links (emoji, date, arrow) was disjointed.
**Action:** Use `aria-label` to provide a cohesive description and explicitly state "opens in new tab" for such links.
