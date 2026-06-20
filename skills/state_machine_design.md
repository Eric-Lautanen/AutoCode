---
name: state-machine-design
description: Use when modeling something that has distinct states and transitions - order workflows, connection lifecycles, UI flows, game states, or protocol implementations. Load when a task involves entities that change state over time, or when if/else chains for state logic are getting out of hand.
---

# State Machine Design

## Overview

A state machine models an entity that can be in one of a finite set of states, transitioning between them via explicit events. When your code has a variable that tracks "status" and a tangle of if/else or switch statements checking what transitions are valid, a state machine brings clarity. Define the states, define the transitions, and let the machine enforce that only valid transitions happen. This skill covers when to use a state machine, how to design one, and how to implement it cleanly.

## When a State Machine Is the Right Model

Use a state machine when:
- An entity has a **finite, known set of states** (not a continuum)
- **Only specific transitions** between states are valid (you can't go from "draft" to "shipped" without "published")
- The current state **determines what actions are available** (a "paused" job can't be "completed")
- You find yourself writing `if status == X and event == Y` chains

Don't use a state machine when:
- State is a simple flag (active/inactive) with no transition constraints
- The number of states is unbounded or data-driven (use a database field instead)
- You need continuous values, not discrete states

## Design Before Code

### Step 1: List All States

Name every possible state the entity can be in. Use domain language:

```
Order: Created → Paid → Packed → Shipped → Delivered
                              ↘ Cancelled (from any state before Shipped)
```

### Step 2: List All Transitions

For each pair of states, define the event that triggers the transition:

| From | Event | To | Guard |
|------|-------|----|-------|
| Created | pay | Paid | payment verified |
| Paid | pack | Packed | items in stock |
| Packed | ship | Shipped | tracking number assigned |
| Shipped | deliver | Delivered | — |
| Created/Paid/Packed | cancel | Cancelled | before shipment |

### Step 3: Define Guards and Actions

- **Guard**: A condition that must be true for the transition to fire. If false, the transition is rejected.
- **Action**: A side effect that happens during the transition (send email, update database, log event).

### Step 4: Draw the Diagram

Draw it before coding. A state diagram is the best documentation:

```
[Created] --pay--> [Paid] --pack--> [Packed] --ship--> [Shipped] --deliver--> [Delivered]
    |                  |                |
    +-------cancel-----+-------cancel---+-------> [Cancelled]
```

## Implementation Patterns

### Enum-Based (Simple, Most Common)

Best for small state machines with a handful of states:

```python
from enum import Enum

class OrderState(Enum):
    CREATED = "created"
    PAID = "paid"
    PACKED = "packed"
    SHIPPED = "shipped"
    DELIVERED = "delivered"
    CANCELLED = "cancelled"

TRANSITIONS = {
    (OrderState.CREATED, "pay"): OrderState.PAID,
    (OrderState.PAID, "pack"): OrderState.PACKED,
    (OrderState.PACKED, "ship"): OrderState.SHIPPED,
    (OrderState.SHIPPED, "deliver"): OrderState.DELIVERED,
    (OrderState.CREATED, "cancel"): OrderState.CANCELLED,
    (OrderState.PAID, "cancel"): OrderState.CANCELLED,
    (OrderState.PACKED, "cancel"): OrderState.CANCELLED,
}

def transition(current: OrderState, event: str) -> OrderState:
    key = (current, event)
    if key not in TRANSITIONS:
        raise InvalidTransition(f"Cannot {event} from {current.value}")
    return TRANSITIONS[key]
```

### Table-Driven (Medium Complexity)

When you need guards and actions alongside transitions:

```typescript
type Transition<S, E> = {
  from: S;
  event: E;
  to: S;
  guard?: (ctx: Context) => boolean;
  action?: (ctx: Context) => void;
};

const transitions: Transition<State, Event>[] = [
  { from: "created", event: "pay", to: "paid",
    guard: (ctx) => ctx.paymentVerified,
    action: (ctx) => sendConfirmationEmail(ctx) },
  { from: "paid", event: "pack", to: "packed",
    action: (ctx) => reserveInventory(ctx) },
  // ...
];
```

### State Pattern (OOP, Complex Machines)

Each state is a class with methods for each possible event:

```python
class OrderStateBase:
    def pay(self, order): raise InvalidTransition()
    def pack(self, order): raise InvalidTransition()
    def ship(self, order): raise InvalidTransition()
    def cancel(self, order): raise InvalidTransition()

class Created(OrderStateBase):
    def pay(self, order):
        order.state = Paid()
    def cancel(self, order):
        order.state = Cancelled()

class Paid(OrderStateBase):
    def pack(self, order):
        order.state = Packed()
    def cancel(self, order):
        order.state = Cancelled()
```

- **Pro**: Adding a new state doesn't require modifying existing states (open/closed principle)
- **Con**: Adding a new event requires modifying every state class
- **Best for**: machines where states change often but events are stable

## Invalid Transitions

**Always reject invalid transitions explicitly.** Never silently ignore them.

- **Raise an exception** (or return an error) with a clear message: `"Cannot 'ship' from state 'created' — must be 'packed' first"`
- **Log the attempt** — invalid transitions often indicate bugs in the caller
- **Don't default to a "safe" state** — silently transitioning to an unexpected state hides bugs

## Hierarchical State Machines

When states share behavior, use hierarchy:

```
Active (parent)
  ├── Running
  └── Paused
Inactive (parent)
  ├── Stopped
  └── Failed
```

- A transition valid in `Active` is valid in both `Running` and `Paused`
- `Paused` can override a parent transition with its own behavior
- Implement with a parent lookup: if the current state doesn't handle the event, check the parent

## Testing State Machines

Test every path:

1. **Every valid transition**: for each (state, event) pair, verify the resulting state
2. **Every guard condition**: test both pass and fail for each guard
3. **Every invalid transition**: verify the correct error is raised
4. **Actions fire**: verify side effects happen during transitions
5. **Full lifecycle**: walk through the complete happy path start to finish
6. **Edge cases**: transition from terminal state, transition with no-op guard, concurrent transitions

```python
def test_cannot_ship_from_created():
    order = Order(state=OrderState.CREATED)
    with pytest.raises(InvalidTransition):
        order.transition("ship")

def test_cancel_from_paid():
    order = Order(state=OrderState.PAID)
    order.transition("cancel")
    assert order.state == OrderState.CANCELLED
```

## Checklist

- [ ] All states enumerated and named in domain language
- [ ] All transitions defined with from, event, to
- [ ] Guards and actions specified for each transition
- [ ] State diagram drawn before coding
- [ ] Invalid transitions raise explicit errors (not silent)
- [ ] Every transition tested, including invalid ones
- [ ] Terminal states identified (no outgoing transitions)
- [ ] Persistence strategy decided (how is current state stored?)
