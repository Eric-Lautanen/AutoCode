---
name: accessibility
description: Use when building any UI that needs to be usable by people with disabilities - keyboard navigation, screen readers, color contrast, focus management, and ARIA. Load when asked to improve accessibility, fix a11y issues, or build any frontend component or page.
---

# Accessibility

## Overview

Accessibility (a11y) means building interfaces that everyone can use — including people who navigate by keyboard, use screen readers, have low vision, or have motor impairments. Accessibility is not a feature you add at the end; it's a property of good HTML and component design. The good news: most accessibility comes free from using semantic HTML correctly. The rest is a small set of patterns you apply consistently.

For component design principles, see `component_design.md`. For UX behavior patterns, see `ux_principles.md`.

## Semantic HTML: Half the Work

The right HTML element does half the accessibility work for free:

| You wrote | Problem | Write instead |
|-----------|---------|---------------|
| `<div onclick="submit()">` | Not focusable, not announced as button | `<button type="submit">` |
| `<div class="nav">` | Not announced as navigation | `<nav>` |
| `<span class="heading">` | Not in heading hierarchy | `<h2>` |
| `<div class="list">` | Not announced as list | `<ul>` with `<li>` |
| `<div class="input">` | Not editable, not labeled | `<input>` with `<label>` |

**Rule**: If it's interactive, use an interactive element. If it's structural, use a semantic element. Only use `<div>` and `<span>` when no semantic element fits.

## Keyboard Navigation

### Every Interactive Element Must Be Reachable

- Tab through the page: every button, link, input, and control must be reachable via Tab key
- Tab order follows visual order (DOM order = visual order in most cases)
- Never use `tabindex > 0` — it disrupts natural tab order. Use `tabindex="0"` only for custom interactive elements
- Use `tabindex="-1"` to make something programmatically focusable but not in tab order

### Every Interactive Element Must Be Operable

| Element | Activate with | Secondary |
|---------|--------------|-----------|
| Button | Enter, Space | — |
| Link | Enter | — |
| Checkbox | Space | — |
| Radio button | Space, Arrow keys | — |
| Text input | Type text | — |
| Select/Dropdown | Space, Arrow keys, Enter | — |
| Tab | Arrow keys | — |
| Dialog | Escape to close | — |

### Focus Management

- **Visible focus ring**: Never `outline: none` without a replacement. Use `:focus-visible` for keyboard-only rings.
- **Modal focus trap**: When a modal opens, focus moves into it. Tab cycles within the modal. Escape closes it. Focus returns to the trigger on close.
- **Dynamic content**: When content changes (page navigation, list filtering, modal close), move focus to the new content or the triggering element.
- **Skip links**: For pages with repetitive navigation, provide a "Skip to main content" link as the first focusable element.

## Screen Readers

Screen readers convert the DOM into speech or braille. They rely on the accessibility tree, which is built from your HTML.

### Images

```html
<!-- Informative image: describe what it shows -->
<img src="chart.png" alt="Sales increased 40% from Q1 to Q3 2024">

<!-- Decorative image: hide from screen reader -->
<img src="divider.png" alt="" role="presentation">

<!-- Complex image: use long description -->
<img src="infographic.png" alt="Company growth overview" aria-describedby="infographic-desc">
<p id="infographic-desc" class="sr-only">Detailed description...</p>
```

### Labels

Every input must have a label. Period.

```html
<!-- Best: explicit label with for attribute -->
<label for="email">Email address</label>
<input id="email" type="email">

<!-- Good: wrapping label -->
<label>
  Email address
  <input type="email">
</label>

<!-- Last resort: aria-label (no visible label) -->
<input type="email" aria-label="Email address">

<!-- Acceptable: aria-labelledby (label is elsewhere on the page) -->
<span id="email-label">Email address</span>
<input type="email" aria-labelledby="email-label">
```

### Headings

- One `<h1>` per page
- Don't skip levels: h1 → h2 → h3 (never h1 → h4)
- Screen reader users navigate by headings — they're the page's table of contents

## ARIA: Only When HTML Isn't Enough

**First rule of ARIA**: Don't use ARIA if a native HTML element provides the semantics you need.

### Common ARIA Attributes

| Attribute | Purpose | Example |
|-----------|---------|---------|
| `aria-label` | Accessible name when no visible label | `<button aria-label="Close">✕</button>` |
| `aria-labelledby` | Reference to visible label element | `aria-labelledby="title-id"` |
| `aria-describedby` | Reference to description element | `aria-describedby="help-text"` |
| `aria-expanded` | Is a collapsible section open? | `aria-expanded="true"` on toggle button |
| `aria-checked` | State of checkbox/radio | `aria-checked="true"` |
| `aria-disabled` | Visually disabled but still focusable | `aria-disabled="true"` (better than `disabled` for custom controls) |
| `aria-hidden` | Hide from screen reader | `aria-hidden="true"` on decorative elements |
| `aria-live` | Announce dynamic content changes | `aria-live="polite"` for status updates |
| `aria-current` | Current item in a set | `aria-current="page"` on active nav link |
| `role` | Override element semantics | `role="alert"` for error messages, `role="dialog"` for modals |

### Live Regions

When content updates dynamically (notifications, search results, form validation):

```html
<!-- Polite: announces when user is idle -->
<div aria-live="polite">3 results found</div>

<!-- Assertive: announces immediately (use sparingly) -->
<div role="alert">Form submission failed</div>
```

## Color Contrast

WCAG AA minimum contrast ratios:

| Element | Minimum ratio |
|---------|-------------|
| Normal text (<18px, <14px bold) | 4.5:1 |
| Large text (≥18px, ≥14px bold) | 3:1 |
| UI components and graphical objects | 3:1 |

**Don't rely on color alone** to convey meaning:
- Error state: red border + error icon + error text (not just red border)
- Required field: asterisk + text "(required)" (not just red asterisk)
- Status indicators: icon + text label (not just colored dot)

## Testing Accessibility

### Manual Testing (Do This First)

1. **Keyboard-only walkthrough**: Unplug your mouse. Can you reach and operate everything?
2. **Screen reader spot check**: Use VoiceOver (Mac), NVDA (Windows), or Orca (Linux) to navigate your page
3. **Zoom to 200%**: Does the layout still work?
4. **High contrast mode**: Does the page still convey information?

### Automated Testing

- **axe DevTools** (browser extension): Catches ~30-40% of issues automatically
- **Lighthouse** (Chrome): Accessibility audit in DevTools
- **eslint-plugin-jsx-a11y** (React): Catches issues during development
- **@testing-library/jest-dom** matchers: `toBeVisible()`, `toHaveAccessibleName()`

**Important**: Automated tools catch syntax issues (missing alt, bad ARIA). They cannot catch meaning issues (wrong alt text, confusing heading order, bad focus management). Manual testing is essential.

## Windows High Contrast Mode

Windows High Contrast Mode (WHCM) is used by many users with low vision. Test your UI with WHCM enabled:

- **Don't rely on background colors alone** - WHCM overrides them. Use borders or outlines for visual distinction.
- **Use the `forced-colors` media query** to adapt when WHCM is active:

```css
@media (forced-colors: active) {
  .card {
    border: 2px solid CanvasText;  /* Visible in any high contrast theme */
  }
}
```

- **Ensure focus indicators remain visible** - WHCM may override your custom focus styles. Use `outline: 2px solid currentColor` for compatibility.

## Checklist

- [ ] All interactive elements use semantic HTML (button, a, input, not div)
- [ ] Every element is reachable and operable by keyboard alone
- [ ] Focus indicator is visible on all interactive elements
- [ ] Every image has appropriate alt text (or alt="" for decorative)
- [ ] Every input has a visible label
- [ ] Headings follow a logical hierarchy without skipping levels
- [ ] Color is not the only way information is conveyed
- [ ] Contrast ratios meet WCAG AA minimums
- [ ] Dynamic content uses aria-live or role="alert"
- [ ] Modals trap focus and return focus on close
- [ ] Automated a11y audit passes (axe, Lighthouse)
- [ ] Manual keyboard walkthrough completed
- [ ] Tested with Windows High Contrast Mode (if targeting Windows users)
