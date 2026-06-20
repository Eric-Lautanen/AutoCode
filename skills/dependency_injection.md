---
name: dependency-injection
description: Use when structuring code to be testable and loosely coupled through dependency injection - passing dependencies in rather than constructing them internally. Load when a task involves making code testable, wiring up application components, using a DI framework (Spring, Angular, InversifyJS, Python injector), or untangling tightly coupled code.
---

# Dependency Injection

## Overview

Dependency injection (DI) is the practice of receiving dependencies from outside rather than creating them inside. A class that constructs its own database connection is tightly coupled to that connection. A class that receives a database connection as a parameter is loosely coupled — you can swap the real connection for a test double, change the implementation, or reconfigure without touching the class. DI is the foundation of testable, modular code.

## The Core Idea

```python
# Bad: class creates its dependency internally
class UserService:
    def __init__(self):
        self.db = PostgresConnection("localhost:5432")  # Tight coupling

    def get_user(self, user_id):
        return self.db.query("SELECT * FROM users WHERE id = %s", user_id)

# Good: dependency is injected
class UserService:
    def __init__(self, db: Database):  # Receives the dependency
        self.db = db

    def get_user(self, user_id):
        return self.db.query("SELECT * FROM users WHERE id = %s", user_id)
```

The `UserService` no longer knows or cares which database it uses. It just needs something that satisfies the `Database` interface.

## Injection Types

### Constructor Injection (Preferred)

Dependencies are provided at construction time. The object is fully initialized and ready to use.

```typescript
class OrderService {
  constructor(
    private paymentGateway: PaymentGateway,
    private notificationService: NotificationService,
    private orderRepository: OrderRepository
  ) {}
}
```

- **Pros**: Dependencies are explicit, immutable after construction, object is always in a valid state
- **Cons**: Constructor can get long with many dependencies (often a sign the class does too much)

### Setter/Property Injection

Dependencies are set after construction via a setter method or property.

```python
class EmailService:
    def __init__(self):
        self.smtp_client = None  # Set later

    def set_smtp_client(self, client):
        self.smtp_client = client
```

- **Pros**: Optional dependencies, can be changed after construction
- **Cons**: Object may be in an incomplete state, dependencies aren't obvious from the constructor

### Method Injection

Dependencies are provided per method call.

```go
func ProcessOrder(ctx context.Context, order Order, gateway PaymentGateway) error {
    return gateway.Charge(ctx, order.Total)
}
```

- **Pros**: Most granular, dependencies are explicit at the call site
- **Cons**: Verbose if the same dependency is passed to many methods

**Default choice**: Constructor injection. Use setter injection only for optional dependencies. Use method injection for context-specific dependencies.

## Depend on Abstractions

The real power of DI comes from depending on interfaces/protocols, not concrete implementations:

```python
# Define the abstraction
from abc import ABC, abstractmethod

class NotificationService(ABC):
    @abstractmethod
    def send(self, recipient: str, message: str) -> None: ...

# Concrete implementation
class EmailNotification(NotificationService):
    def send(self, recipient: str, message: str) -> None:
        smtp.send(recipient, message)

# Another implementation
class SlackNotification(NotificationService):
    def send(self, recipient: str, message: str) -> None:
        slack.post(recipient, message)

# Consumer depends on the abstraction
class AlertManager:
    def __init__(self, notifier: NotificationService):  # Not EmailNotification
        self.notifier = notifier
```

**Rule**: The type of an injected dependency should be an interface/protocol, not a concrete class. This is what makes swapping implementations trivial.

## Composition Root

All the wiring happens in one place: the composition root. This is where you construct concrete objects and inject them into each other.

```python
# composition_root.py (or main.py, or app.py)

def create_app():
    # Construct concrete implementations
    db = PostgresConnection(config.database_url)
    user_repo = UserRepository(db)
    email_service = EmailNotification(config.smtp_host)
    slack_service = SlackNotification(config.slack_webhook)

    # Wire them together
    user_service = UserService(user_repo, email_service)
    alert_manager = AlertManager(slack_service)
    order_service = OrderService(user_service, alert_manager)

    return App(user_service, order_service)
```

**Key points**:
- The composition root is the only place that knows about concrete types
- Everything else depends on interfaces
- The composition root should be small and obvious — it's a declaration of how the app is wired, not logic

## DI Containers/Frameworks

DI containers automate the wiring. You register bindings, and the container resolves dependencies automatically.

| Framework | Language | How it works |
|-----------|----------|-------------|
| Spring | Java | Annotations (`@Autowired`, `@Component`), auto-scanning |
| Angular DI | TypeScript | Provider tokens, hierarchical injectors |
| InversifyJS | TypeScript | Decorators, container bindings |
| Guice | Java | Annotations, modules |
| Python injector | Python | Decorators, provider bindings |

### When to Use a Container

- **Large applications** with many services and complex wiring
- **Framework-mandated** (Spring, Angular — you're using it whether you want to or not)
- **Cross-cutting concerns** that need to be woven in (logging, transactions, auth)

### When NOT to Use a Container

- **Small applications** with <10 services — manual wiring is clearer
- **When it obscures the wiring**: If you can't trace how a dependency is provided by reading the composition root, the container is hiding too much
- **When it becomes a service locator** (see anti-patterns below)

### Manual DI Is Underrated

For most applications, manual DI in a composition root is simpler, more explicit, and easier to debug than a container:

```typescript
// Manual: clear, debuggable, no magic
const db = new PostgresConnection(config.dbUrl);
const userRepo = new UserRepository(db);
const emailService = new SMTPEmailService(config.smtp);
const userService = new UserService(userRepo, emailService);
```

## Testing Benefit

DI's primary practical benefit: testability.

```python
# Production wiring
user_service = UserService(real_db, real_email)

# Test wiring
fake_db = InMemoryDatabase()
mock_email = MockNotificationService()
user_service = UserService(fake_db, mock_email)

# Test: no real database, no real emails
result = user_service.create_user("alice@example.com")
assert fake_db.find_by_email("alice@example.com") is not None
assert mock_email.sent_count == 1
```

Without DI, you'd need to mock module-level imports, use monkeypatching, or rely on fragile test setup. With DI, you just pass different objects.

## Common Anti-Patterns

### Service Locator

```python
# Bad: pulling dependencies from a global registry
class UserService:
    def get_user(self, id):
        db = ServiceLocator.get("database")  # Hidden dependency
        return db.query(...)
```

- **Why it's bad**: Dependencies are hidden — you can't tell from the class signature what it needs. It's DI in reverse (pull instead of push).
- **Fix**: Use constructor injection. Make dependencies explicit.

### Injecting the Container Itself

```python
# Bad: injecting the container
class UserService:
    def __init__(self, container: DIContainer):
        self.db = container.get("database")
```

- **Why it's bad**: The class now depends on the entire container, not just its specific dependencies. It's the service locator pattern in disguise.
- **Fix**: Inject only the specific dependencies the class needs.

### Over-Injection (Too Many Dependencies)

```python
# Bad: 8 dependencies = class does too much
class DashboardService:
    def __init__(self, user_repo, order_repo, analytics_repo,
                 email, cache, config, logger, clock):
        ...
```

- **Why it's bad**: A class with many dependencies is doing too many things. It's a god object.
- **Fix**: Split the class. Each class should have 1-3 dependencies. If you need more, extract a collaborator.

### Interface That Matches Only One Implementation

```python
# Bad: interface is just the concrete class renamed
class IPostgresUserRepository(ABC):  # "I" prefix is a smell
    @abstractmethod
    def query_postgres(self, sql: str) -> list: ...
```

- **Why it's bad**: The interface is coupled to the implementation. You can't swap it for a different database.
- **Fix**: Design the interface from the consumer's perspective: `UserRepository.find_by_id(id) -> User`

## Checklist

- [ ] Dependencies are injected, not constructed internally
- [ ] Constructor injection is the default (setter/method only when appropriate)
- [ ] Dependencies are interfaces/protocols, not concrete types
- [ ] Composition root is the single place where wiring happens
- [ ] DI container used only when it adds value (large apps, framework-mandated)
- [ ] No service locator anti-pattern (no global registry lookups)
- [ ] No more than 3-4 dependencies per class (split if more)
- [ ] Interfaces designed from the consumer's perspective, not the implementation's
