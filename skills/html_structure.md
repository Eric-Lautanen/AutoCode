---
name: html-structure
description: Use when writing HTML - semantic element selection, document structure, forms, tables, meta tags, and making markup that is accessible, SEO-friendly, and well-structured. Load when building a page from scratch, reviewing HTML for correctness, or when accessibility or SEO is a concern.
---

# HTML Structure

## Overview

HTML is the foundation of every web page. Well-structured HTML means better accessibility (screen readers understand the page), better SEO (search engines understand the content), and less CSS/JS needed (semantic elements have built-in behavior and styling). This skill covers the HTML patterns that make pages correct, accessible, and maintainable.

For accessibility specifics beyond HTML, see `accessibility.md`. For DOM manipulation, see `javascript_dom.md`.

## Document Structure

### The Boilerplate

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta name="description" content="Concise page description for SEO">
  <title>Page Title — Site Name</title>
  <link rel="stylesheet" href="styles.css">
</head>
<body>
  <header>...</header>
  <main>...</main>
  <footer>...</footer>
</body>
</html>
```

**Every page needs**:
- `<!DOCTYPE html>` — standards mode rendering
- `lang` attribute — screen readers use this for pronunciation
- `charset` — always UTF-8
- `viewport` meta — responsive behavior on mobile
- `<title>` — required, unique per page, important for SEO and tab identification

## Semantic Elements

Use the right element for the job. Semantic elements carry meaning; `<div>` carries none.

| Element | Purpose | Not this |
|---------|---------|----------|
| `<header>` | Introductory content or navigation for a section | `<div class="header">` |
| `<nav>` | Navigation links | `<div class="nav">` |
| `<main>` | Primary page content (one per page) | `<div class="content">` |
| `<article>` | Self-contained composition (blog post, comment) | `<div class="post">` |
| `<section>` | Thematic grouping with a heading | `<div class="section">` |
| `<aside>` | Tangentially related content (sidebar) | `<div class="sidebar">` |
| `<footer>` | Footer for nearest sectioning element | `<div class="footer">` |
| `<figure>` | Self-contained content with caption | `<div class="image">` |
| `<figcaption>` | Caption for `<figure>` | `<p class="caption">` |
| `<time>` | Date/time value | `<span class="date">` |
| `<address>` | Contact information | `<div class="contact">` |
| `<mark>` | Highlighted/relevant text | `<span class="highlight">` |

### Landmark Regions

Screen readers navigate by landmarks:

```html
<body>
  <header>
    <nav aria-label="Main navigation">...</nav>
  </header>
  <main>
    <article>...</article>
    <aside aria-label="Related articles">...</aside>
  </main>
  <footer>...</footer>
</body>
```

**Rule**: If you have two `<nav>` elements on a page, add `aria-label` to distinguish them.

## Headings

- **One `<h1>` per page** — the page's main topic
- **Don't skip levels**: h1 → h2 → h3 (never h1 → h4)
- **Headings create a document outline** — screen reader users jump between headings

```html
<h1>Company Blog</h1>
  <h2>Latest Posts</h2>
    <h3>How to Build Accessible Forms</h3>
    <h3>Understanding CSS Grid</h3>
  <h2>Categories</h2>
    <h3>Frontend</h3>
    <h3>Backend</h3>
```

**Anti-pattern**: Using headings for visual sizing. Use CSS for size, headings for structure.

## Forms

### Label + Input Pairing

```html
<!-- Best: explicit label with for -->
<label for="email">Email address</label>
<input id="email" type="email" name="email" required>

<!-- Good: wrapping label -->
<label>
  Email address
  <input type="email" name="email" required>
</label>
```

### Input Types

Use the correct type for built-in validation and mobile keyboard optimization:

| Type | Mobile keyboard | Built-in validation |
|------|-----------------|-------------------|
| `text` | Default | None |
| `email` | Shows @ key | Validates email format |
| `tel` | Phone keypad | None |
| `url` | URL-optimized | Validates URL format |
| `number` | Numeric | Validates number |
| `search` | Search enter key | None |
| `date` | Date picker | Validates date |

### Fieldset for Groups

```html
<fieldset>
  <legend>Shipping method</legend>
  <label><input type="radio" name="shipping" value="standard"> Standard (5-7 days)</label>
  <label><input type="radio" name="shipping" value="express"> Express (2-3 days)</label>
  <label><input type="radio" name="shipping" value="overnight"> Overnight</label>
</fieldset>
```

### Required and Disabled

```html
<input type="text" name="username" required>          <!-- Must fill in -->
<input type="text" name="username" required aria-required="true">  <!-- Explicit ARIA -->
<select name="country" disabled>                       <!-- Can't interact -->
<input type="submit" value="Save" disabled>           <!-- Greyed out button -->
```

## Tables

Only use tables for tabular data — never for layout.

```html
<table>
  <caption>Monthly revenue by region</caption>
  <thead>
    <tr>
      <th scope="col">Region</th>
      <th scope="col">January</th>
      <th scope="col">February</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <th scope="row">North America</th>
      <td>$1.2M</td>
      <td>$1.3M</td>
    </tr>
  </tbody>
  <tfoot>
    <tr>
      <th scope="row">Total</th>
      <td>$3.4M</td>
      <td>$3.6M</td>
    </tr>
  </tfoot>
</table>
```

**Key elements**: `<caption>` (accessible title), `<thead>`/`<tbody>`/`<tfoot>`, `scope="col"` or `scope="row"` on `<th>`.

## Images

```html
<!-- Informative image: describe what it shows -->
<img src="chart.png" alt="Revenue increased 40% from Q1 to Q3 2024" width="600" height="400">

<!-- Decorative image: hide from screen readers -->
<img src="divider.png" alt="" role="presentation">

<!-- Responsive images -->
<img src="photo-800.jpg"
     srcset="photo-400.jpg 400w, photo-800.jpg 800w, photo-1200.jpg 1200w"
     sizes="(max-width: 600px) 100vw, 800px"
     alt="Team at the conference"
     width="800" height="600"
     loading="lazy"
     decoding="async">
```

**Rules**:
- Every `<img>` has an `alt` attribute (empty string for decorative)
- `width` and `height` prevent layout shift (CLS)
- `loading="lazy"` for below-the-fold images
- Above-the-fold images: no lazy loading (add `fetchpriority="high"` for LCP image)

## Links

```html
<!-- Meaningful link text — not "click here" -->
<a href="/docs/getting-started">Read the getting started guide</a>

<!-- External links: warn users and prevent security issues -->
<a href="https://external-site.com" target="_blank" rel="noopener noreferrer">
  View on external site
</a>

<!-- Skip link: first focusable element on the page -->
<a href="#main-content" class="skip-link">Skip to main content</a>
```

**`rel="noopener noreferrer"`**: Always add when using `target="_blank"`. Prevents the new page from accessing `window.opener` (security) and referrer information (privacy).

## Meta Tags

### Essential

```html
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<meta name="description" content="Concise page description (150-160 chars for SEO)">
<title>Page Title — Site Name</title>
```

### Open Graph (Social Sharing)

```html
<meta property="og:title" content="Page Title">
<meta property="og:description" content="Page description for social previews">
<meta property="og:image" content="https://example.com/og-image.jpg">
<meta property="og:url" content="https://example.com/page">
<meta property="og:type" content="website">
```

### Other Important

```html
<link rel="canonical" href="https://example.com/page">  <!-- Prevents duplicate content SEO issues -->
<meta name="robots" content="index, follow">             <!-- Search engine directives -->
<link rel="icon" href="/favicon.ico">                     <!-- Browser tab icon -->
```

## Windows-Specific HTML Notes

### Windows High Contrast Mode
Support Windows High Contrast Mode with proper meta tags and CSS:

```html
<!-- No special meta tag needed, but ensure CSS uses system colors -->
<style>
  @media (forced-colors: active) {
    .button {
      border: 2px solid ButtonText;
    }
    .button:focus {
      outline: 3px solid Highlight;
    }
  }
</style>
```

### Windows Tile and Pinned Site
Configure Windows tile for pinned sites:

```html
<!-- Windows tile configuration -->
<meta name="msapplication-TileColor" content="#2b5797">
<meta name="msapplication-TileImage" content="/mstile-144x144.png">
<meta name="msapplication-config" content="/browserconfig.xml">
```

```xml
<!-- browserconfig.xml -->
<?xml version="1.0" encoding="utf-8"?>
<browserconfig>
  <msapplication>
    <tile>
      <square150x150logo src="/mstile-150x150.png"/>
      <TileColor>#2b5797</TileColor>
    </tile>
  </msapplication>
</browserconfig>
```

### Windows Font Stack
Include Windows system fonts for native feel:

```html
<style>
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  }
</style>
```

### Windows Touch Targets
Ensure adequate touch targets for Windows tablets:

```html
<style>
  /* Minimum 44px touch target */
  .touch-target {
    min-width: 44px;
    min-height: 44px;
  }
  
  /* Windows tablet-specific */
  @media (pointer: coarse) {
    .button {
      min-height: 48px;
      padding: 12px 24px;
    }
  }
</style>
```

## Checklist

- [ ] Document has DOCTYPE, lang, charset, viewport meta
- [ ] Semantic elements used (nav, main, article, section — not div for everything)
- [ ] One h1 per page, no skipped heading levels
- [ ] Every input has a label (via for attribute or wrapping)
- [ ] Every image has alt text (or alt="" for decorative)
- [ ] External links have `rel="noopener noreferrer"` with `target="_blank"`
- [ ] Tables used only for tabular data, with caption and scope attributes
- [ ] Open Graph meta tags for social sharing
- [ ] Canonical URL specified
- [ ] Windows tile configuration (if applicable)
- [ ] Windows High Contrast Mode supported
- [ ] Windows touch targets adequate for tablets
