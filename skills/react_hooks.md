# React Hooks — useState, useEffect, Custom Hooks

## useState

```jsx
const [count, setCount] = useState(0)
setCount(c => c + 1)   // prefer updater function
```

## useEffect

```jsx
useEffect(() => {
  fetchData().then(setData)
  return () => cleanup()   // cleanup on unmount
}, [id])   // re-run when id changes
```

## Custom hook pattern

```jsx
function useLocalStorage(key, initial) {
  const [value, setValue] = useState(() => {
    return JSON.parse(localStorage.getItem(key)) ?? initial
  })
  useEffect(() => {
    localStorage.setItem(key, JSON.stringify(value))
  }, [key, value])
  return [value, setValue]
}
```
