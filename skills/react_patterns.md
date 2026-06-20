---
name: react-patterns
description: Use when writing or debugging React code - components, hooks, state management, effects, context, and performance optimization. Load when any task involves React components, JSX, or React-specific patterns like useEffect or custom hooks.
---

# React Patterns

## Overview

React's declarative model is powerful but has sharp edges around effects, state, and rendering. The core principle: **React renders what your state describes — if the UI is wrong, the state is wrong.** Fix the state, and the UI fixes itself.

## Component Design

### Single Responsibility
One component, one job. If a component does too much, split it:

```jsx
// BAD — does too much
function UserDashboard({ userId }) {
    const user = useUser(userId);
    const orders = useOrders(userId);
    const settings = useSettings(userId);
    // 200 lines of JSX...
}

// GOOD — split by responsibility
function UserDashboard({ userId }) {
    return (
        <div>
            <UserProfile userId={userId} />
            <UserOrders userId={userId} />
            <UserSettings userId={userId} />
        </div>
    );
}
```

### Props vs. State
- **Props**: Data passed in from parent. Component doesn't own it.
- **State**: Data the component owns and manages internally.

### Controlled vs. Uncontrolled
- **Controlled**: React state is the source of truth (`value` + `onChange`)
- **Uncontrolled**: The DOM is the source of truth (`ref` to get the value)

**Prefer controlled components.** They make the data flow explicit and testable.

## Hooks

### useState
```jsx
const [count, setCount] = useState(0);
```
- State updates are batched — multiple `setState` calls in one event handler trigger one re-render
- Use the functional updater when new state depends on old: `setCount(c => c + 1)`

### useEffect
```jsx
// Run once on mount
useEffect(() => {
    fetchData();
}, []);

// Run when dependency changes
useEffect(() => {
    fetchUserData(userId);
}, [userId]);

// Cleanup on unmount
useEffect(() => {
    const subscription = subscribe();
    return () => subscription.unsubscribe();
}, []);
```

### useEffect Pitfalls

**Missing dependencies:**
```jsx
// BAD — stale closure: 'userId' is captured from first render
useEffect(() => {
    fetchUser(userId);
}, []);  // Missing userId in deps

// GOOD — include all dependencies
useEffect(() => {
    fetchUser(userId);
}, [userId]);
```

**Infinite loops:**
```jsx
// BAD — setUsers triggers re-render, which triggers effect, which calls setUsers...
useEffect(() => {
    fetchUsers().then(setUsers);
});  // No dependency array = runs every render

// GOOD — empty dependency array = runs once
useEffect(() => {
    fetchUsers().then(setUsers);
}, []);
```

**Missing cleanup:**
```jsx
// BAD — subscription leaks memory
useEffect(() => {
    const ws = new WebSocket(url);
    ws.onmessage = handleMessage;
}, [url]);  // Old WebSocket never closed!

// GOOD — cleanup old subscription
useEffect(() => {
    const ws = new WebSocket(url);
    ws.onmessage = handleMessage;
    return () => ws.close();
}, [url]);
```

### useRef
```jsx
// Mutable value that persists across renders without causing re-renders
const timerRef = useRef(null);

useEffect(() => {
    timerRef.current = setInterval(tick, 1000);
    return () => clearInterval(timerRef.current);
}, []);
```

### useCallback and useMemo
```jsx
// useCallback — memoize a function (prevent unnecessary child re-renders)
const handleClick = useCallback(() => doSomething(id), [id]);

// useMemo — memoize a computed value
const sortedItems = useMemo(() => items.sort(compareFn), [items]);
```

**Measure before applying.** Premature memoization adds complexity without benefit. Only use when profiling shows a real performance problem.

## Custom Hooks

Extract reusable stateful logic:

```jsx
// Custom hook
function useDebounce(value, delay) {
    const [debouncedValue, setDebouncedValue] = useState(value);
    useEffect(() => {
        const timer = setTimeout(() => setDebouncedValue(value), delay);
        return () => clearTimeout(timer);
    }, [value, delay]);
    return debouncedValue;
}

// Usage
const searchTerm = useDebounce(inputValue, 300);
```

**Naming convention:** `use` prefix (e.g., `useAuth`, `useLocalStorage`, `useDebounce`)

## Context

### When to Use
- Theme (dark/light mode)
- Auth state (current user)
- Locale/language settings

### When It's Overkill
- Passing props 1-2 levels deep — just pass props
- Data that changes frequently — context re-renders all consumers

### When Prop Drilling Is Worse
- Passing the same prop through 4+ intermediate components
- The intermediate components don't use the prop, they just pass it down

## Performance

### React.memo
```jsx
const ExpensiveList = React.memo(function ExpensiveList({ items }) {
    return items.map(item => <Item key={item.id} {...item} />);
});
```
Only helps when the parent re-renders frequently but the component's props don't change. Measure first.

### Key Prop Misuse
```jsx
// BAD — using index as key causes bugs when list items change
{items.map((item, index) => <Item key={index} {...item} />)}

// GOOD — use a stable, unique ID
{items.map(item => <Item key={item.id} {...item} />)}
```

**Why index keys are bad:** When items are added/removed/ reordered, React matches by index, not by identity. This causes components to receive wrong state and produce subtle bugs.

## State Management

| Approach | When to use |
|----------|-------------|
| Local state (`useState`) | Default choice — most state belongs here |
| Context | Shared state across many components (theme, auth) |
| Zustand | Medium-complexity global state — simpler than Redux |
| Redux | Large apps with complex state interactions, time-travel debugging |

**Start with local state.** Only add a global store when you have a measured need.

## Common Mistakes

- **State mutation**: Never mutate state directly. `state.push(item)` → `setState([...state, item])`
- **Stale closures**: Functions in useEffect capturing old values from a previous render
- **Key prop misuse**: Using array index as key in dynamic lists
- **Over-using useEffect**: If you can compute something from props/state, don't put it in useEffect — compute it during render
- **Premature memoization**: Adding `useMemo`/`useCallback` everywhere without measuring

## Anti-Patterns

- **Giant components.** If it's 200+ lines, split it.
- **Prop drilling through 5+ levels.** Use context or a state manager.
- **Effects for derived state.** `useEffect(() => setFullName(first + last), [first, last])` → just compute `const fullName = first + last` during render.
- **Not cleaning up effects.** Subscriptions, timers, and event listeners leak memory.
- **Index as key in dynamic lists.** Causes incorrect rendering on reorder/add/remove.
