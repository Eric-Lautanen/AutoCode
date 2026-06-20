---
name: css-styling
description: Use when styling elements with CSS - colors, typography, spacing, borders, shadows, transforms, transitions, animations, and pseudo-classes. Load when a task involves making something look a specific way, implementing a visual design, or fixing visual bugs.
---

# CSS Styling

## Overview

CSS styling is about the visual properties of elements: colors, typography, spacing, borders, shadows, and animations. This skill covers the properties and patterns you use most often, with practical guidance on avoiding common pitfalls. For layout (flexbox, grid, positioning), see `css_layout.md`. For organizing CSS at scale, see `css_architecture.md`.

## Box Model

Every element is a rectangular box with four layers:

```
┌─────────────────────────────┐
│          margin             │
│  ┌───────────────────────┐  │
│  │        border         │  │
│  │  ┌─────────────────┐  │  │
│  │  │     padding      │  │  │
│  │  │  ┌───────────┐  │  │  │
│  │  │  │  content  │  │  │  │
│  │  │  └───────────┘  │  │  │
│  │  └─────────────────┘  │  │
│  └───────────────────────┘  │
└─────────────────────────────┘
```

### box-sizing: border-box

```css
/* Apply globally — makes width/height include padding and border */
*, *::before, *::after {
  box-sizing: border-box;
}
```

Without `border-box`, `width: 200px` + `padding: 20px` = 240px total. With `border-box`, it's 200px total (padding is inside). **Always use `border-box`.**

## Typography

### Font Stack

```css
body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto,
               Oxygen, Ubuntu, Cantarell, sans-serif;
  font-size: 16px;
  line-height: 1.5;
  color: var(--color-text-primary);
}

code, pre {
  font-family: 'Fira Code', 'Consolas', 'Monaco', monospace;
}
```

### Key Properties

```css
.text {
  font-size: 1rem;           /* 16px base, scales with root */
  font-weight: 400;          /* 400=normal, 500=medium, 600=semibold, 700=bold */
  font-style: normal;        /* italic for emphasis */
  line-height: 1.5;         /* Unitless: relative to font-size */
  letter-spacing: -0.01em;  /* Tighter for large text */
  text-transform: none;     /* uppercase, lowercase, capitalize */
  text-decoration: none;     /* underline, line-through */
  text-align: left;         /* left, center, right, justify */
  white-space: normal;      /* nowrap, pre, pre-wrap */
  text-overflow: ellipsis;  /* Truncate with "..." — needs overflow:hidden + white-space:nowrap */
  word-break: break-word;   /* Break long words */
}
```

### Responsive Type Scale

```css
h1 { font-size: clamp(2rem, 5vw, 3.5rem); }
h2 { font-size: clamp(1.5rem, 3vw, 2.5rem); }
h3 { font-size: clamp(1.25rem, 2vw, 1.75rem); }
```

## Colors

### Color Formats

```css
.element {
  /* Hex — most common */
  color: #1a1a2e;

  /* RGB with alpha */
  color: rgba(26, 26, 46, 0.8);

  /* HSL — most intuitive for variations */
  color: hsl(240, 30%, 14%);

  /* oklch — modern, perceptually uniform */
  color: oklch(0.25 0.05 270);

  /* Custom properties for theming */
  color: var(--color-text-primary);
  background: var(--color-bg-secondary);
}
```

### Opacity vs. Alpha

```css
/* opacity: affects the entire element AND its children */
.card { opacity: 0.5; }  /* Card and all its text are 50% transparent */

/* rgba/alpha: affects only the specific property */
.card {
  background: rgba(0, 0, 0, 0.5);  /* Background is 50% transparent */
  color: #000;                       /* Text is fully opaque */
}
```

**Rule**: Use alpha channels on specific properties. Avoid `opacity` unless you want everything inside to be transparent.

## Spacing

### Margin vs. Padding

| | Margin | Padding |
|---|---|---|
| **Space** | Outside the border | Inside the border |
| **Background** | Transparent | Inherits element's background |
| **Collapsing** | Vertical margins collapse | Never collapses |
| **Use for** | Separating elements from each other | Space between content and border |

### Collapsing Margins

Vertical margins between adjacent elements collapse — the larger margin wins, they don't add:

```css
/* These two elements have 30px between them, not 50px */
.element-1 { margin-bottom: 30px; }
.element-2 { margin-top: 20px; }  /* Collapses: max(30, 20) = 30px */
```

**Fix**: Use `gap` in flex/grid containers (no collapsing), or use padding instead of margin, or use only one direction (`margin-bottom` on all siblings).

### gap (Flexbox and Grid)

```css
.container {
  display: flex;
  gap: 16px;           /* Spacing between items — no collapsing, no margin hacks */
  row-gap: 24px;       /* Vertical gap */
  column-gap: 16px;    /* Horizontal gap */
}
```

**Prefer `gap` over margins** for spacing between sibling elements.

## Borders and Shadows

```css
.element {
  /* Border shorthand */
  border: 1px solid var(--color-border-default);
  border-radius: 8px;

  /* Individual sides */
  border-top: 2px solid var(--color-primary);
  border-bottom: none;

  /* Box shadow: x-offset y-offset blur spread color */
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);           /* Subtle */
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);            /* Medium */
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.15);         /* Elevated */
  box-shadow: 0 0 0 2px var(--color-primary);            /* Ring (focus) */

  /* Multiple shadows */
  box-shadow: 0 1px 2px rgba(0,0,0,0.05), 0 4px 8px rgba(0,0,0,0.1);

  /* Text shadow */
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}
```

## Transforms

```css
.element {
  /* Translate: move without affecting layout */
  transform: translateX(10px);
  transform: translateY(-50%);     /* Center vertically trick */
  transform: translate(10px, -20px);

  /* Scale */
  transform: scale(1.05);          /* 5% larger */
  transform: scaleX(0.5);          /* Half width */

  /* Rotate */
  transform: rotate(45deg);

  /* Combine (order matters!) */
  transform: translateX(10px) rotate(45deg) scale(1.1);

  /* Transform origin */
  transform-origin: top left;      /* Default: center center */
}
```

**Key point**: Transforms don't affect layout — other elements don't reflow. This makes them ideal for animations.

## Transitions

Smooth changes between property values:

```css
.button {
  background: var(--color-primary);
  transition: background 0.2s ease, transform 0.15s ease;
}

.button:hover {
  background: var(--color-primary-hover);
  transform: translateY(-1px);
}

/* Shorthand: property duration timing-function delay */
transition: all 0.3s ease 0s;

/* Timing functions */
transition-timing-function: ease;        /* Default, slow start/end */
transition-timing-function: ease-in-out;
transition-timing-function: ease-out;    /* Good for entrances */
transition-timing-function: linear;
transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1);  /* Material Design standard */
```

**Rule**: Only transition properties that are cheap to animate — `transform` and `opacity`. Avoid transitioning `width`, `height`, `top`, `left` (causes layout reflow).

## Animations

```css
@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to   { opacity: 1; transform: translateY(0); }
}

.element {
  animation: fadeIn 0.3s ease-out forwards;
}

/* Shorthand: name duration timing-function delay iteration-count direction fill-mode */
animation: fadeIn 0.3s ease-out 0s 1 normal forwards;
```

### Performance

Only animate `transform` and `opacity` for smooth 60fps. These are composited on the GPU and don't trigger layout.

```css
/* Good: GPU-composited */
.fade { transition: opacity 0.3s ease; }
.slide { transition: transform 0.3s ease; }

/* Bad: triggers layout on every frame */
.resize { transition: width 0.3s ease; }
.move { transition: top 0.3s ease; }
```

## Pseudo-Classes and Pseudo-Elements

### Pseudo-Classes (Element States)

```css
a:hover { text-decoration: underline; }
a:focus-visible { outline: 2px solid var(--color-primary); }
input:disabled { opacity: 0.5; }
input:required { border-left: 3px solid var(--color-primary); }
input:invalid { border-color: var(--color-error); }
li:first-child { margin-top: 0; }
li:last-child { margin-bottom: 0; }
li:nth-child(odd) { background: var(--color-bg-secondary); }
```

### Pseudo-Elements (Generated Content)

```css
/* Decorative content */
.label::before { content: "•"; margin-right: 8px; }
.required::after { content: "*"; color: var(--color-error); }

/* Clearfix (for float-based layouts — rarely needed with flex/grid) */
.container::after { content: ""; display: table; clear: both; }

/* Styling placeholder text */
input::placeholder { color: var(--color-text-tertiary); }

/* Selection highlight */
::selection { background: var(--color-primary); color: white; }
```

## Checklist

- [ ] `box-sizing: border-box` applied globally
- [ ] Typography uses a consistent scale, not arbitrary sizes
- [ ] Colors use semantic custom properties, not hardcoded values
- [ ] `gap` used for spacing between siblings instead of margin hacks
- [ ] Only `transform` and `opacity` animated for performance
- [ ] `:focus-visible` used for focus styles (not `:focus`)
- [ ] `prefers-reduced-motion` respected for animations
- [ ] No `!important` (if needed, specificity is wrong — fix the source)
