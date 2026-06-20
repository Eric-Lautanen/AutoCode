---
name: frontend-basics
description: Use when working on frontend code - HTML structure, CSS layout, JavaScript DOM manipulation, event handling, form validation, or browser APIs. Load when a task involves building or editing UI in a browser context, whether vanilla JS or a framework.
---

# Frontend Basics

## Overview

Frontend development means building for the browser — an environment you don't control, running on devices you can't predict. The core principle: **build for the user's device, not your development machine.** Performance, accessibility, and resilience matter more than pixel-perfect designs on one browser.

## HTML Semantics

### Use the Right Element
```html
<!-- BAD — div soup, no semantics -->
<div class="button" onclick="submit()">Submit</div>
<div class="nav"><div class="nav-item">Home</div></div>

<!-- GOOD — semantic elements -->
<button type="submit">Submit</button>
<nav><a href="/">Home</a></nav>
```

**Why semantics matter:**
- Screen readers understand semantic elements
- Search engines understand semantic elements
- Native behavior comes free (keyboard support, form submission, focus management)

### Key Semantic Elements
| Element | Use for |
|---------|---------|
| `<nav>` | Navigation blocks |
| `<main>` | Primary content (one per page) |
| `<article>` | Self-contained content (blog post, card) |
| `<section>` | Thematic grouping with a heading |
| `<aside>` | Sidebar, tangentially related content |
| `<header>` / `<footer>` | Introductory / closing content |
| `<button>` | Clickable actions (not `<div>` or `<a>`) |
| `<a>` | Navigation to another URL (not `<div>` or `<button>`) |

## CSS Layout

### Flexbox vs. Grid
- **Flexbox**: 1D layout — a row or a column of items
- **Grid**: 2D layout — rows and columns simultaneously

```css
/* Flexbox — distribute items in a row */
.navbar { display: flex; justify-content: space-between; align-items: center; }

/* Grid — define rows and columns */
.dashboard { display: grid; grid-template-columns: 250px 1fr; grid-template-rows: auto 1fr; }
```

**Rule:** Use flexbox for component-level layout (navbar, card content). Use grid for page-level layout (sidebar + main, dashboard grid).

## CSS Specificity

### How It Works
Specificity determines which rule wins when multiple rules target the same element:

1. Inline styles (highest)
2. `#id` selectors
3. `.class`, `[attr]`, `:pseudo-class` selectors
4. `element`, `::pseudo-element` selectors (lowest)

**Why `!important` is a smell:** It breaks the cascade. If you need `!important`, your specificity is probably wrong. Fix the specificity, don't override it.

### BEM Naming Convention
```css
/* Block__Element--Modifier */
.card {}              /* Block */
.card__title {}      /* Element */
.card--featured {}   /* Modifier */
.card__title--large {} /* Element modifier */
```

**Benefits:** Flat specificity (all single-class), self-documenting names, no naming conflicts.

## JS DOM

### querySelector and querySelectorAll
```javascript
const el = document.querySelector('.card');           // First match
const all = document.querySelectorAll('.card');        // All matches (NodeList, not Array)
const closest = el.closest('.container');              // Nearest ancestor matching selector
const matches = el.matches('.active');                // Does this element match?
```

### Event Delegation
Attach one listener to a parent instead of many to children:

```javascript
// BAD — one listener per item (memory, performance)
items.forEach(item => item.addEventListener('click', handleClick));

// GOOD — one listener on the parent
container.addEventListener('click', (e) => {
    const item = e.target.closest('.item');
    if (item) handleClick(item);
});
```

### Avoiding Memory Leaks
- Remove event listeners when elements are removed from the DOM
- Use `{ once: true }` for one-time listeners
- Be careful with closures that capture DOM references — they prevent garbage collection

## Forms

### Input Types and Validation
```html
<input type="email" required>           <!-- Browser validates email format -->
<input type="number" min="0" max="100">  <!-- Browser validates range -->
<input type="password" minlength="8">    <!-- Browser enforces minimum length -->
<input type="url" pattern="https://.*">  <!-- Browser validates pattern -->
```

### HTML5 Built-in vs. JS Validation
- **Use HTML5 for**: Required fields, type validation (email, number, URL), min/max, pattern
- **Use JS for**: Cross-field validation, async validation (checking if username is taken), custom error messages

**Always validate on the server too.** Client-side validation is UX, not security.

### Prevent Default
```javascript
form.addEventListener('submit', (e) => {
    e.preventDefault();  // Prevent page reload
    const formData = new FormData(form);
    submitData(Object.fromEntries(formData));
});
```

## Async in the Browser

### fetch API
```javascript
// Basic GET
const response = await fetch('/api/users');
if (!response.ok) throw new Error(`HTTP ${response.status}`);
const users = await response.json();

// POST with body
const response = await fetch('/api/users', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name: 'Alice' }),
});
```

### Error Handling
```javascript
try {
    const data = await fetchUserData();
    renderData(data);
} catch (error) {
    if (error instanceof TypeError) {
        showError('Network error — check your connection');
    } else {
        showError(`Something went wrong: ${error.message}`);
    }
}
```

## Browser Storage

| Storage | Capacity | Persistence | Use for |
|---------|----------|-------------|---------|
| `localStorage` | ~5MB | Survives tab close | User preferences, theme |
| `sessionStorage` | ~5MB | Tab only | Form state, temporary data |
| Cookies | 4KB | Configurable expiry | Auth tokens (HttpOnly) |
| IndexedDB | Large | Persistent | Offline data, complex client-side data |

**Never store sensitive data in localStorage.** It's accessible to any JS on the page (XSS risk).

## Common Pitfalls

- **Layout thrashing**: Reading a layout property (offsetHeight) then writing a style in a loop forces the browser to recalculate layout repeatedly. Batch reads before writes.
- **Synchronous XHR**: Never use `XMLHttpRequest` synchronously — it freezes the UI thread.
- **Blocking the main thread**: Long computations block rendering. Use Web Workers for CPU-heavy work.
- **Not handling loading states**: Show a loading indicator while data is being fetched.
- **Not handling error states**: Show an error message when the fetch fails.

## Anti-Patterns

- **Using divs for everything.** `<div onclick>` instead of `<button>`, `<div class="nav">` instead of `<nav>`.
- **Inline styles over classes.** Hard to maintain, high specificity, no reusability.
- **Not preventing default on form submit.** Page reloads on every form submission.
- **Storing tokens in localStorage.** Vulnerable to XSS. Use HttpOnly cookies.
- **Not handling the offline state.** The network will fail. Handle it gracefully.
