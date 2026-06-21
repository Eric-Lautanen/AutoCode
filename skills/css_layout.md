---
name: css-layout
description: Use when implementing any CSS layout - flexbox, grid, positioning, responsive design, or when something isn't laying out the way it should. Load when a task involves arranging elements on a page, building a responsive layout, fixing a layout bug, or implementing a design that requires precise spatial control.
---

# CSS Layout

## Overview

CSS layout is about controlling where elements appear and how they relate to each other spatially. The two main systems — flexbox and grid — handle 90% of layout needs. Understanding when to use each, how positioning works, and how to make layouts responsive eliminates most layout bugs. This skill covers the layout patterns you'll use most often.

For visual styling (colors, typography, shadows), see `css_styling.md`. For CSS architecture and methodology, see `css_architecture.md`.

## Flexbox

Flexbox is for **one-dimensional** layout — distributing items along a single axis (row or column).

### Core Concepts

```css
.container {
  display: flex;

  /* Direction: which axis is "main" */
  flex-direction: row;        /* left→right (default) */
  flex-direction: column;     /* top→bottom */

  /* Main axis alignment */
  justify-content: flex-start | center | flex-end | space-between | space-around | space-evenly;

  /* Cross axis alignment */
  align-items: stretch | flex-start | center | flex-end | baseline;

  /* Wrapping */
  flex-wrap: nowrap | wrap;
  gap: 16px;                  /* Spacing between items (replaces margin hacks) */
}
```

### Item Sizing

```css
.item {
  /* Grow: how much this item should grow to fill free space */
  flex-grow: 0;    /* Don't grow (default) */
  flex-grow: 1;    /* Grow equally with other grow:1 items */

  /* Shrink: how much this item should shrink when space is tight */
  flex-shrink: 1;  /* Shrink equally (default) */
  flex-shrink: 0;  /* Don't shrink — keep my size */

  /* Basis: initial size before growing/shrinking */
  flex-basis: auto;  /* Size from content or width/height */
  flex-basis: 200px; /* Start at 200px, then grow/shrink */

  /* Shorthand: grow shrink basis */
  flex: 1;          /* = flex: 1 1 0% — grow equally, shrink, start at 0 */
  flex: 0 0 200px;  /* Fixed 200px, don't grow or shrink */
  flex: auto;       /* = flex: 1 1 auto — grow/shrink based on content */
}
```

### Common Patterns

```css
/* Center a single item */
.center { display: flex; justify-content: center; align-items: center; }

/* Sidebar + main (sidebar fixed, main fills) */
.layout { display: flex; }
.sidebar { flex: 0 0 250px; }
.main { flex: 1; }

/* Equal-width columns */
.columns { display: flex; gap: 16px; }
.columns > * { flex: 1; }

/* Push last item to the right */
.nav { display: flex; }
.nav .spacer { flex: 1; }  /* Add empty spacer before last item */
```

## Grid

Grid is for **two-dimensional** layout — controlling both rows and columns simultaneously.

### Basic Grid

```css
.grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);   /* 3 equal columns */
  grid-template-columns: 250px 1fr 250px;  /* Fixed, flex, fixed */
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); /* Responsive cards */
  gap: 16px;
}
```

### Named Areas

```css
.layout {
  display: grid;
  grid-template-areas:
    "header  header  header"
    "sidebar content aside"
    "footer  footer  footer";
  grid-template-columns: 250px 1fr 200px;
  grid-template-rows: auto 1fr auto;
  min-height: 100vh;
}

.header  { grid-area: header; }
.sidebar { grid-area: sidebar; }
.content { grid-area: content; }
.aside   { grid-area: aside; }
.footer  { grid-area: footer; }
```

### Spanning and Placement

```css
.item {
  grid-column: 1 / 3;        /* Span columns 1-2 */
  grid-column: 1 / -1;       /* Span full width */
  grid-column: span 2;        /* Span 2 columns from auto position */
  grid-row: span 2;           /* Span 2 rows */
}
```

### Auto-Placement

```css
/* Masonry-like: items fill available space */
.grid {
  grid-auto-flow: dense;  /* Fill gaps with smaller items */
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
}
```

## When to Use Which

| Layout need | Use | Why |
|-------------|-----|-----|
| Navigation bar (horizontal items) | Flexbox | 1D row of items |
| Centering content | Flexbox | Simple, one-liner |
| Card grid (responsive columns) | Grid | 2D, auto-fill handles responsiveness |
| Sidebar + main layout | Either | Grid if you want template areas; flexbox if simpler |
| Holy grail layout (header/sidebar/content/aside/footer) | Grid | Template areas make it clear |
| Form with label + input pairs | Grid | Align labels and inputs in columns |
| Equal-height columns | Grid | Grid rows are equal height by default |
| Wrapping tag/chip list | Flexbox | `flex-wrap: wrap` with `gap` |

**Rule of thumb**: If you're thinking about rows AND columns at the same time, use grid. If you're thinking about one axis, use flexbox.

## Positioning

| Value | Behavior | Use case |
|-------|----------|----------|
| `static` | Default flow | — |
| `relative` | Offset from normal position, stays in flow | Fine-tuning position, positioning context for absolute children |
| `absolute` | Removed from flow, positioned relative to nearest positioned ancestor | Tooltips, dropdowns, badges |
| `fixed` | Positioned relative to viewport | Sticky headers, modals, floating action buttons |
| `sticky` | Scrolls normally until threshold, then sticks | Table headers, section nav |

### Stacking Context

`z-index` only works within the same stacking context. If your `z-index: 9999` isn't working, check if a parent created a new stacking context (via `transform`, `opacity < 1`, `filter`, `will-change`, or `position` + `z-index`).

**Rule**: Avoid high `z-index` values. Use a scale: 1-10 for normal layers, 100 for modals, 1000 for tooltips.

## Responsive Design

### Mobile-First

Write base styles for mobile, then add complexity for larger screens:

```css
/* Base: mobile */
.grid { display: flex; flex-direction: column; gap: 16px; }

/* Tablet */
@media (min-width: 768px) {
  .grid { flex-direction: row; }
}

/* Desktop */
@media (min-width: 1024px) {
  .grid { display: grid; grid-template-columns: 250px 1fr; }
}
```

### Fluid Sizing

```css
/* clamp(): responsive value with min and max */
h1 {
  font-size: clamp(1.5rem, 4vw, 3rem);  /* Scales with viewport, bounded */
}

/* Container queries: respond to parent size, not viewport */
.card-container { container-type: inline-size; }

@container (min-width: 400px) {
  .card { flex-direction: row; }
}
```

### Breakpoints

| Name | Width | Target |
|------|-------|--------|
| sm | 640px | Large phones |
| md | 768px | Tablets |
| lg | 1024px | Small laptops |
| xl | 1280px | Desktops |
| 2xl | 1536px | Large screens |

## Common Layout Patterns

### Holy Grail

```css
.page {
  display: grid;
  grid-template: "header header" auto
                 "sidebar content" 1fr
                 "footer footer" auto
                 / 250px 1fr;
  min-height: 100vh;
}
```

### Sticky Header

```css
.header {
  position: sticky;
  top: 0;
  z-index: 10;
  background: var(--color-bg);  /* Must have background to cover content */
}
```

### Centered Content with Max Width

```css
.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 0 var(--spacing-md);
}
```

## Debugging Layouts

- **Browser DevTools**: Inspect element → see the box model, computed flex/grid values
- **Outline everything**: `* { outline: 1px solid red; }` — quickly see element boundaries
- **Flexbox debugger**: DevTools shows flex arrows and alignment
- **Grid debugger**: DevTools shows grid lines and area names
- **Check the parent**: Most layout bugs are in the container, not the child

## Windows-Specific Layout Notes

### High DPI Displays on Windows
Windows handles DPI scaling differently from macOS. Layouts may appear at different sizes:

```css
/* Use relative units for DPI-aware layouts */
.container {
  /* Good: relative units scale with DPI */
  padding: 1rem;
  max-width: 120ch;
}

/* Avoid fixed pixel sizes for text */
.text {
  font-size: 1rem;  /* Good: scales with user preferences */
  /* NOT: font-size: 16px; */
}
```

### Windows Snap Layouts
Windows 11 introduced snap layouts. Test your responsive design at these sizes:

```css
/* Common Windows snap sizes */
@media (max-width: 600px) { /* Phone / small snap */ }
@media (min-width: 601px) and (max-width: 1024px) { /* Tablet / medium snap */ }
@media (min-width: 1025px) { /* Desktop / large snap */ }
```

### Scrollbar Space on Windows
Windows scrollbars take up space (unlike macOS overlay scrollbars). Account for this:

```css
/* Reserve space for scrollbar to prevent layout shift */
.scrollable {
  overflow-y: auto;
  scrollbar-gutter: stable;  /* Reserve space for scrollbar */
}
```

### Windows Taskbar and Safe Areas
When building full-screen or PWA layouts on Windows:

```css
/* Account for Windows taskbar in full-screen layouts */
.full-screen {
  height: 100vh;
  /* Windows taskbar may reduce available height */
  padding-bottom: env(safe-area-inset-bottom, 0);
}
```

## Checklist

- [ ] Flexbox for 1D layouts, grid for 2D layouts
- [ ] `gap` used for spacing instead of margin hacks
- [ ] Responsive: mobile-first with min-width media queries
- [ ] No fixed heights that break with content changes
- [ ] `position: sticky` for headers/nav instead of `position: fixed`
- [ ] z-index values are reasonable (not 99999)
- [ ] Container queries used where component responds to parent size
- [ ] Layout tested at all breakpoints
- [ ] Windows DPI scaling handled (relative units)
- [ ] Windows snap layouts tested
- [ ] Scrollbar space reserved on Windows
