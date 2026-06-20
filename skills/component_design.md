---
name: component-design
description: Use when designing or building reusable UI components - deciding on props, variants, states, and composition patterns. Load when building a component library, adding a new component, or when an existing component is getting too complex or hard to reuse.
---

# Component Design

## Overview

A well-designed component does one thing, has a minimal API surface, handles every state it can be in, and composes well with other components. A poorly designed component has too many props, too many boolean flags, doesn't handle loading or error states, and can't be reused without copying and modifying. This skill covers the principles for designing components that are reusable, accessible, and maintainable — applicable to React, Vue, Svelte, Angular, or any component framework.

For visual design principles (spacing, color, typography), see `ui_design_fundamentals.md`. For accessibility specifics, see `accessibility.md`.

## Single Responsibility

One component, one job. If a component does two things, split it.

**Signs a component does too much:**
- It has more than 8-10 props
- It has props that are only used in certain combinations
- You're passing a "mode" or "variant" that completely changes what it renders
- The render function has deeply nested conditionals

**Fix**: Extract sub-components. A `UserCard` that shows avatar, name, bio, and stats should be composed from `Avatar`, `UserInfo`, and `UserStats` components.

## Props API Design

### Minimal Surface

Expose only what consumers need. Every prop is a commitment — you can't remove it without a breaking change.

```tsx
// Bad: too many low-level props
<Button
  bgColor={string}
  textColor={string}
  borderColor={string}
  paddingX={number}
  paddingY={number}
  fontSize={number}
/>

// Good: semantic props, implementation details handled internally
<Button
  variant="primary" | "secondary" | "danger"
  size="sm" | "md" | "lg"
/>
```

### Sensible Defaults

Every optional prop should have a sensible default. A consumer should be able to render the component with zero props and get something reasonable:

```tsx
<Button /> // Renders a medium-sized secondary button — works, looks fine
```

### Avoid Boolean Prop Explosion

Boolean flags multiply the states you have to test and style:

```tsx
// Bad: 2^4 = 16 possible combinations
<Button primary outline small disabled />

// Good: variant + size covers the same ground with fewer combinations
<Button variant="primary-outline" size="sm" disabled />
```

## Variants

Use a single `variant` (or `intent`, `kind`) prop instead of many booleans:

```tsx
type ButtonVariant = "primary" | "secondary" | "outline" | "ghost" | "danger";
type ButtonSize = "sm" | "md" | "lg";

interface ButtonProps {
  variant?: ButtonVariant;  // default: "secondary"
  size?: ButtonSize;        // default: "md"
  disabled?: boolean;
  loading?: boolean;
  children: React.ReactNode;
  onClick?: () => void;
}
```

**Why**: `variant` is explicit, enumerable, and easy to style-map. Boolean combinations are implicit and combinatorial.

## States

Every interactive component has states. Design for all of them:

| State | What to show | Common omission |
|-------|-------------|-----------------|
| Default | Normal appearance | — |
| Hover | Visual feedback on mouse-over | Often forgotten for buttons |
| Focus | Keyboard focus indicator | **Critical for accessibility** |
| Active/Pressed | While being clicked | Subtle visual change |
| Disabled | Can't interact, reduced opacity | Don't just grey out — make it obviously inactive |
| Loading | Action in progress | Show spinner or skeleton, disable interaction |
| Error | Something went wrong | Show error state, not just nothing |
| Empty | No data to display | Show helpful message, not blank space |

**Rule**: If a component can be in a state, it must look different in that state. A disabled button that looks the same as an enabled one is a bug.

## Composition Over Configuration

Prefer composing simple components over adding configuration props:

```tsx
// Bad: configuration-heavy
<Card
  showHeader={true}
  headerTitle="Settings"
  showFooter={true}
  footerAction="Save"
  showAvatar={true}
  avatarUrl="/me.jpg"
/>

// Good: composition with slots
<Card>
  <Card.Header>
    <Avatar src="/me.jpg" />
    <h3>Settings</h3>
  </Card.Header>
  <Card.Body>
    {children}
  </Card.Body>
  <Card.Footer>
    <Button variant="primary">Save</Button>
  </Card.Footer>
</Card>
```

**Benefits**: Consumers control layout, add anything to slots, and the Card component stays simple.

## Accessibility

Every component must be accessible by default:

- **Interactive elements**: Use `<button>` for buttons, `<a>` for links. Don't make a `<div>` clickable.
- **Labels**: Every input has a visible label or `aria-label`
- **Keyboard**: Focusable, operable with Enter/Space, logical tab order
- **Focus ring**: Visible focus indicator. Never `outline: none` without a replacement.
- **ARIA**: Only when HTML semantics aren't enough. `aria-expanded`, `aria-checked`, `aria-disabled`
- **Screen reader text**: Use `.sr-only` class for text that's hidden visually but read aloud

See `accessibility.md` for comprehensive guidance.

## Naming

- **Clear, not clever**: `Button` not `Btn`, `Modal` not `Popup`, `Dropdown` not `Droppy`
- **Consistent**: If you use `variant` in one component, use `variant` in all of them (not `kind`, `type`, `mode`)
- **Domain-accurate**: `InvoiceStatus` not `StatusBadge` (the component is about invoices, not badges)
- **Avoid abbreviations**: `Navigation` not `Nav`, `Avatar` not `Avtr`

## Documentation

Show every variant and state with an example:

```tsx
// Button.stories.tsx — or whatever your docs system uses
export const AllVariants = () => (
  <Stack gap="md">
    <Button variant="primary">Primary</Button>
    <Button variant="secondary">Secondary</Button>
    <Button variant="danger">Danger</Button>
    <Button variant="outline">Outline</Button>
    <Button variant="ghost">Ghost</Button>
  </Stack>
);

export const AllStates = () => (
  <Stack gap="md">
    <Button>Default</Button>
    <Button disabled>Disabled</Button>
    <Button loading>Loading</Button>
  </Stack>
);
```

**Rule**: If it's not documented, it doesn't exist. Consumers won't discover props by reading source code.

## Checklist

- [ ] Component has a single responsibility
- [ ] Props API is minimal — semantic variants, not low-level style props
- [ ] Every state has a distinct visual appearance (default, hover, focus, disabled, loading, error)
- [ ] Composition pattern used for complex components (slots/children over config props)
- [ ] Accessible by default: semantic HTML, keyboard operable, focus visible
- [ ] Named clearly and consistently with other components
- [ ] All variants and states documented with examples
- [ ] Sensible defaults — works with zero props
