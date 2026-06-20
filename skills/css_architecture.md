---
name: css-architecture
description: Use when organizing CSS in a large project - naming conventions, file structure, avoiding specificity wars, scoping styles, and deciding between methodologies like BEM, utility-first (Tailwind), or CSS Modules. Load when starting a new project's CSS strategy, when stylesheets are getting hard to maintain, or when styles are unexpectedly overriding each other.
---

# CSS Architecture

## Overview

CSS is easy to write and hard to maintain. Without a system, styles grow into a tangled mess of increasing specificity, `!important` battles, and fear of changing anything because you don't know what else it affects. CSS architecture is about organizing styles so they're predictable, maintainable, and scalable. The key decisions: how to name things, how to scope styles, and how to structure files.

For layout patterns, see `css_layout.md`. For visual styling properties, see `css_styling.md`. For design tokens, see `design_tokens.md`.

## Specificity

### How It's Calculated

Specificity determines which rule wins when multiple rules target the same element:

| Selector | Specificity | Example |
|----------|------------|---------|
| Inline style | 1,0,0,0 | `style="color: red"` |
| ID | 0,1,0,0 | `#header` |
| Class/attr/pseudo-class | 0,0,1,0 | `.nav`, `[type="text"]`, `:hover` |
| Element/pseudo-element | 0,0,0,1 | `div`, `::before` |

`#nav .item a:hover` = 0,1,2,1 (one ID, two classes, one element)

### Why Specificity Causes Problems

1. **Escalation**: To override a high-specificity rule, you write an even higher one. This spirals.
2. **Unpredictable overrides**: A global `.button` class might be overridden by `#sidebar .button` somewhere, and you can't tell why your button looks wrong.
3. **`!important` as a weapon**: When specificity wars are lost, `!important` is the nuclear option. It makes the rule impossible to override without another `!important`.

### Keep Specificity Flat

**Goal**: All selectors should have specificity of 0,0,1,0 (one class) or at most 0,0,2,0 (two classes).

```css
/* Bad: escalating specificity */
#header .nav .nav-item a { color: blue; }

/* Good: flat specificity */
.nav-link { color: blue; }
```

**Rules**:
- Never use IDs in CSS selectors (they're fine in JS for `getElementById`)
- Never nest more than 2-3 levels deep
- Never use `!important` except for utility classes that must always win
- If you need to override, add a more specific class, not a longer selector

## BEM Naming

Block__Element--Modifier:

```css
/* Block: standalone component */
.card { ... }

/* Element: part of the block */
.card__title { ... }
.card__body { ... }
.card__footer { ... }

/* Modifier: variation of block or element */
.card--featured { ... }
.card__title--large { ... }
.card--dark { ... }
```

### When BEM Works

- Medium-to-large projects with many components
- Teams that need consistent naming conventions
- When you want flat specificity with clear relationships

### When BEM Is Overkill

- Small projects (<10 components)
- When you're using utility-first CSS (Tailwind) or CSS Modules
- When the naming feels like bureaucracy more than clarity

### BEM Tips

- **Block names should be unique**: Don't have two `.card` blocks with different styles
- **Don't nest elements**: `.card__body__title` is wrong. Use `.card__title` even if it's inside body
- **Modifiers are additional classes**: `<div class="card card--featured">`, not just `card--featured`

## Utility-First (Tailwind)

Instead of writing custom CSS, compose styles from small utility classes:

```html
<button class="bg-blue-500 hover:bg-blue-600 text-white font-semibold
               py-2 px-4 rounded-lg shadow-md transition-colors">
  Save
</button>
```

### Tradeoffs

| Pro | Con |
|-----|-----|
| No naming decisions | HTML is verbose |
| No specificity issues | Hard to read long class lists |
| Consistent design (uses token scale) | Custom components need `@apply` or extraction |
| Fast to build | Difficult to understand complex components from HTML alone |
| Purged unused styles = tiny CSS | Learning curve for the utility vocabulary |

### When to Use

- Projects where speed of development matters
- Teams that want design consistency without a component library
- When you want tiny CSS bundles (unused utilities are purged)

### When Not to Use

- When you need highly custom, branded designs that don't fit a utility scale
- When your team prefers semantic class names for readability
- When you're building a component library for others to consume

## CSS Modules

Locally scoped class names at build time:

```css
/* Button.module.css */
.primary { background: blue; color: white; }
.secondary { background: gray; color: black; }
```

```jsx
import styles from './Button.module.css';

function Button({ variant }) {
  return <button className={styles[variant]}>Click</button>;
}
```

- **Locally scoped**: `.primary` compiles to `.Button_primary_1a2b3` — no global collision
- **Composition**: `composes: secondary;` to extend another class
- **Works well with component frameworks**: React, Vue, Svelte

### When to Use

- Component-based architectures (React, Vue)
- When you want scoped styles without a utility framework
- When you prefer writing CSS (not utility classes)

## CSS-in-JS

| Library | Runtime? | Approach |
|---------|---------|----------|
| styled-components | Yes | Tagged template literals |
| Emotion | Yes | Object styles or tagged templates |
| Stitches | Minimal | Utility-like API with tokens |
| Vanilla Extract | No (build-time) | Type-safe CSS-in-TS |
| Linaria | No (build-time) | Zero-runtime CSS extraction |

### Tradeoffs

- **Pro**: Styles are co-located with components, scoped by default, can use JS variables
- **Con**: Runtime cost (for runtime libs), bundle size, learning curve, poor SSR performance (some libs)
- **Trend**: Moving toward zero-runtime solutions (Vanilla Extract, Linaria) for performance

## File Organization

```
styles/
  tokens.css          # Design tokens (custom properties)
  reset.css           # CSS reset or normalize
  base.css            # Base element styles (body, h1-h6, a, code)
  utilities.css       # Utility classes (if not using Tailwind)

components/
  Button/
    Button.css        # Component styles (or .module.css)
    Button.tsx
  Card/
    Card.css
    Card.tsx
```

**Rules**:
- One CSS file per component, in the same directory
- Global styles only for resets, tokens, and base element styles
- No component styles in global files

## Resets and Normalization

### CSS Reset

```css
/* Minimal reset */
*, *::before, *::after { box-sizing: border-box; }
* { margin: 0; padding: 0; }
```

- Removes all default browser styles
- You style everything from scratch
- More control, more work

### Normalize

```css
/* Import normalize.css */
@import 'normalize.css';
```

- Makes browser defaults consistent (not removed)
- Less work, less control
- Good starting point for most projects

**Recommendation**: Use a reset for projects with a strong design system. Use normalize for projects that want sensible defaults.

## Custom Properties (Variables)

```css
:root {
  --color-primary: #3B82F6;
  --spacing-md: 16px;
}

.component {
  color: var(--color-primary);
  padding: var(--spacing-md);
}

/* Override in a specific context */
.dark .component {
  --color-primary: #60A5FA;  /* Lighter blue for dark mode */
}
```

**Key features**:
- Cascade and inherit like other CSS properties
- Can be overridden at any level (component, element)
- Enable theming without preprocessor compilation
- See `design_tokens.md` for a full token system

## Checklist

- [ ] Specificity is flat — no IDs in selectors, no deep nesting
- [ ] Naming convention chosen (BEM, utility-first, CSS Modules, or CSS-in-JS)
- [ ] One CSS file per component, co-located with the component
- [ ] Global styles limited to reset, tokens, and base element styles
- [ ] No `!important` except for utility override classes
- [ ] Custom properties used for theming and token values
- [ ] CSS reset or normalize included as the first stylesheet
