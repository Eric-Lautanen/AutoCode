---
name: design-tokens
description: Use when setting up or working with a design token system - colors, spacing, typography, shadows, and border radii defined as named variables shared across code and design. Load when starting a new UI project, building a component library, or when hardcoded values are creating inconsistency across a codebase.
---

# Design Tokens

## Overview

Design tokens are named constants for visual decisions — colors, spacing, typography, shadows, border radii — stored in a single source of truth and shared across code and design tools. Instead of `#3B82F6` scattered through 50 files, you write `var(--color-primary)`. When the brand color changes, you update one token. Tokens replace magic numbers with meaningful names, enforce consistency, and enable theming (including dark mode) without touching component code.

For visual design principles, see `ui_design_fundamentals.md`. For CSS architecture, see `css_architecture.md`.

## What Design Tokens Are

A design token is a named value that represents a visual decision:

```css
/* Not a token — magic number */
.button { background: #3B82F6; padding: 8px 16px; border-radius: 6px; }

/* Tokens — named, meaningful, changeable */
.button {
  background: var(--color-primary);
  padding: var(--spacing-sm) var(--spacing-md);
  border-radius: var(--radius-md);
}
```

**Key insight**: Tokens are named by **purpose**, not by value. `--color-primary` not `--blue-500`. When the brand changes from blue to purple, you update the token, not every component.

## Token Hierarchy

Three layers, from abstract to specific:

### 1. Global Tokens (Raw Values)

The primitive palette. These are your raw materials — never use them directly in components.

```css
:root {
  /* Colors */
  --blue-500: #3B82F6;
  --blue-600: #2563EB;
  --gray-50: #F9FAFB;
  --gray-900: #111827;
  --red-500: #EF4444;
  --green-500: #22C55E;

  /* Spacing */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --space-8: 32px;

  /* Typography */
  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --font-mono: 'Fira Code', 'Consolas', monospace;
  --text-sm: 14px;
  --text-base: 16px;
  --text-lg: 20px;
}
```

### 2. Semantic Tokens (Purpose-Named)

Map global tokens to purposes. Components reference these, never global tokens.

```css
:root {
  /* Text */
  --color-text-primary: var(--gray-900);
  --color-text-secondary: var(--gray-500);
  --color-text-disabled: var(--gray-300);
  --color-text-inverse: var(--gray-50);

  /* Background */
  --color-bg-primary: #FFFFFF;
  --color-bg-secondary: var(--gray-50);
  --color-bg-elevated: #FFFFFF;

  /* Borders */
  --color-border-default: var(--gray-200);
  --color-border-focus: var(--blue-500);

  /* Semantic */
  --color-error: var(--red-500);
  --color-success: var(--green-500);
  --color-primary: var(--blue-500);
  --color-primary-hover: var(--blue-600);

  /* Spacing */
  --spacing-xs: var(--space-1);
  --spacing-sm: var(--space-2);
  --spacing-md: var(--space-4);
  --spacing-lg: var(--space-6);
  --spacing-xl: var(--space-8);
}
```

### 3. Component Tokens (Optional)

Override semantic tokens for specific components when needed:

```css
.button {
  --button-bg: var(--color-primary);
  --button-bg-hover: var(--color-primary-hover);
  --button-padding: var(--spacing-sm) var(--spacing-md);
  --button-radius: var(--radius-md);
}
```

## Dark Mode with Tokens

Dark mode is trivial with semantic tokens — just redefine them:

```css
:root { /* Light mode (default) */
  --color-text-primary: var(--gray-900);
  --color-bg-primary: #FFFFFF;
  --color-bg-secondary: var(--gray-50);
  --color-border-default: var(--gray-200);
}

[data-theme="dark"] {
  --color-text-primary: var(--gray-50);
  --color-bg-primary: var(--gray-900);
  --color-bg-secondary: var(--gray-800);
  --color-border-default: var(--gray-700);
}
```

Components don't change at all — they reference semantic tokens, which automatically resolve to the right values.

## Token Formats

| Format | When to use | How to consume |
|--------|------------|----------------|
| **CSS custom properties** | Web projects | `var(--token-name)` in any CSS |
| **JS/TS constants** | JS-heavy projects, React Native | `import { tokens } from './tokens'` |
| **JSON** | Multi-platform sharing (web + mobile + design) | Input to Style Dictionary or similar |
| **SCSS variables** | Legacy SCSS projects | `$token-name` — migrate to custom properties when possible |

### JSON Source of Truth (Multi-Platform)

```json
{
  "color": {
    "primary": { "value": "{blue.500}", "type": "color" },
    "text": {
      "primary": { "value": "{gray.900}", "type": "color" },
      "secondary": { "value": "{gray.500}", "type": "color" }
    }
  },
  "spacing": {
    "sm": { "value": "8px", "type": "spacing" },
    "md": { "value": "16px", "type": "spacing" }
  }
}
```

Use **Style Dictionary** or **Figma Tokens** to generate CSS, JS, Swift, and Kotlin outputs from this single source.

## Spacing Scale

Use a consistent base unit. The most common: **4px base**.

```
Token          Value    Use case
spacing-1      4px      Icon gaps, inline spacing
spacing-2      8px      Tight element spacing
spacing-3      12px     Related elements
spacing-4      16px     Default component padding
spacing-5      20px     Comfortable spacing
spacing-6      24px     Section separation
spacing-8      32px     Major section gaps
spacing-10     40px     Large separation
spacing-12     48px     Page-level spacing
spacing-16     64px     Hero-level spacing
```

**Rule**: If you're writing a value that isn't in this scale, you're doing it wrong. Use the nearest token.

## Color System

### Primitive Palette

Define a full palette for each hue (50-900 scale):

```
blue-50   → lightest (backgrounds)
blue-100  → light background
blue-200  → light border
blue-300  → muted text
blue-400  → icons, decorative
blue-500  → default (primary brand)
blue-600  → hover state
blue-700  → active/pressed
blue-800  → dark text on light bg
blue-900  → darkest (dark mode surfaces)
```

### Semantic Color Categories

| Category | Tokens needed |
|----------|-------------|
| **Text** | primary, secondary, disabled, inverse |
| **Background** | primary, secondary, elevated, inverted |
| **Border** | default, focus, error |
| **Brand** | primary, primary-hover, primary-active |
| **Semantic** | error, error-bg, success, success-bg, warning, warning-bg |

## Typography Tokens

```css
:root {
  /* Family */
  --font-family-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --font-family-mono: 'Fira Code', 'Consolas', monospace;

  /* Size scale */
  --font-size-xs: 12px;
  --font-size-sm: 14px;
  --font-size-base: 16px;
  --font-size-lg: 20px;
  --font-size-xl: 24px;
  --font-size-2xl: 32px;
  --font-size-3xl: 48px;

  /* Weight */
  --font-weight-normal: 400;
  --font-weight-medium: 500;
  --font-weight-semibold: 600;
  --font-weight-bold: 700;

  /* Line height */
  --line-height-tight: 1.25;
  --line-height-normal: 1.5;
  --line-height-relaxed: 1.75;
}
```

## Keeping Tokens in Sync

- **Single source of truth**: One file (or set of files) defines all tokens. Everything else is generated.
- **Don't edit generated files**: If tokens are generated from JSON or Figma, don't hand-edit the CSS output.
- **CI check**: Fail the build if token files are out of date (run generator, check for uncommitted changes).
- **Figma integration**: Use Figma Tokens plugin or Variables to keep design and code in sync. Designers change tokens in Figma, developers pull the changes.

## Checklist

- [ ] Tokens are named by purpose, not by value
- [ ] Three-layer hierarchy: global → semantic → component
- [ ] Components reference semantic tokens, never global tokens
- [ ] Dark mode implemented by redefining semantic tokens
- [ ] Spacing uses a consistent scale (4px base)
- [ ] Color system has primitive palette + semantic layer
- [ ] Typography tokens cover family, size, weight, line-height
- [ ] Single source of truth with CI enforcement
