# Testing Practices — Unit, Integration, E2E

## Test pyramid

- **Unit tests** — fast, isolated, cover logic
- **Integration tests** — test real I/O, DB, API
- **E2E tests** — full user flows, slowest

## Arrange-Act-Assert

```python
# arrange
store = Inventory()
store.add_item("widget", 5)

# act
result = store.purchase("widget", 2)

# assert
assert result.success
assert store.quantity("widget") == 3
```

## Mocks vs fakes

- **Mocks** — verify interactions (was method called?)
- **Fakes** — lightweight implementations (in-memory DB)
- Prefer fakes over mocks when possible
