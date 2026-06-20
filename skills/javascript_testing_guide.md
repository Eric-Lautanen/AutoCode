# JavaScript Testing Guide — Vitest & Playwright

## Vitest setup

```js
// vitest.config.js
import { defineConfig } from 'vitest/config'
export default defineConfig({ test: { globals: true } })
```

## Unit test

```js
import { describe, it, expect } from 'vitest'
describe('utils', () => {
  it('adds numbers', () => {
    expect(add(1, 2)).toBe(3)
  })
})
```

## Playwright e2e

```js
test('homepage loads', async ({ page }) => {
  await page.goto('/')
  await expect(page.locator('h1')).toHaveText('Welcome')
})
```
