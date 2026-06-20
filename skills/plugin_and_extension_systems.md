---
name: plugin-and-extension-systems
description: Use when designing or implementing a plugin system, extension API, hook system, or any architecture where external code plugs into a core system. Load when a task involves making a system extensible, writing a plugin for an existing system, or designing an API for third-party extensions.
---

# Plugin and Extension Systems

## Overview

Plugin systems let external code extend a core application without modifying it. Done well, they enable rich ecosystems (VS Code extensions, WordPress plugins, Babel presets). Done poorly, they create tight coupling, breaking changes, and security nightmares. The key decisions: what capabilities to expose, how plugins are discovered and loaded, how to isolate them, and how to maintain backward compatibility across versions.

## Plugin Patterns

### Hooks/Events

The simplest pattern: the core emits events, plugins subscribe.

```python
# Core emits
emitter.emit("before_save", document=document)

# Plugin subscribes
@emitter.on("before_save")
def validate_document(document):
    if not document.title:
        raise ValidationError("Title required")
```

- **Best for**: cross-cutting concerns (logging, validation, notifications)
- **Limitation**: plugins can't easily coordinate; order of execution matters
- **Tip**: make hooks async-aware if your system is async

### Middleware Chains

Plugins process a request/response in sequence, each can short-circuit:

```javascript
// Each middleware receives the request and a "next" function
app.use(async (req, next) => {
  req.startTime = Date.now();
  const res = await next(req);
  console.log(`Took ${Date.now() - req.startTime}ms`);
  return res;
});
```

- **Best for**: request processing pipelines (HTTP, CLI commands, data transforms)
- **Limitation**: linear — every plugin sees the same data shape
- **Tip**: provide a way to skip remaining middleware (short-circuit)

### Strategy Pattern

Core delegates a specific operation to a pluggable implementation:

```rust
trait Storage {
    fn save(&self, data: &Data) -> Result<(), Error>;
    fn load(&self, id: &str) -> Result<Data, Error>;
}

// Core depends on the trait, plugins provide implementations
```

- **Best for**: swapping implementations (storage backends, auth providers, renderers)
- **Limitation**: one plugin per extension point (unless you chain them)
- **Tip**: provide a default implementation so the system works without plugins

### Dynamic Loading

Load code at runtime from a directory or registry:

```python
# Discover and load plugin modules
for module_info in pkgutil.iter_modules(plugin_dir):
    module = importlib.import_module(f"plugins.{module_info.name}")
    if hasattr(module, "register"):
        module.register(registry)
```

- **Best for**: user-installed extensions (editor plugins, CLI extensions)
- **Limitation**: security and stability risks from arbitrary code execution
- **Tip**: validate plugin metadata before loading; catch import errors gracefully

## Interface Design

### Stable Plugin APIs

The plugin API is a contract. Breaking it breaks every plugin at once.

- **Version the API**: expose `api_version` so the core can reject incompatible plugins
- **Prefer composition over inheritance**: plugins implement small interfaces, not deep hierarchies
- **Document every public method**: plugin authors can't read your source code easily
- **Deprecate before removing**: mark old methods deprecated for one major version, then remove

### What to Expose vs. Keep Internal

| Expose to plugins | Keep internal |
|---|---|
| Data models and types | Internal implementation details |
| Extension point interfaces | Database schema |
| Read-only access to state | Direct state mutation |
| Configuration values | Other plugins' internal state |
| Event/notification system | Core lifecycle management |

**Principle**: expose the minimum that lets plugins do useful work. Every exposed API is a maintenance commitment.

## Discovery

### File-Based (Scan a Directory)

```python
# Each plugin is a file or folder in a known directory
plugins/
  auth_oauth.py
  auth_saml.py
  export_csv.py
```

- Simple, predictable, easy to debug
- Works for local plugins and self-hosted systems
- Load order: alphabetical, or by a `priority` field in metadata

### Registry-Based

```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "entrypoint": "index.js",
  "minCoreVersion": "2.0.0"
}
```

- Plugins declare metadata in a manifest (package.json, plugin.yaml, pyproject.toml)
- Core reads manifests, validates compatibility, then loads entrypoints
- Enables marketplace/registry features (search, install, update)

### Built-in Registration

```python
# Plugin registers itself when imported
def register(registry):
    registry.add_hook("before_save", validate_document)
    registry.add_command("export", export_handler)
```

- Simplest to implement
- Plugin must be imported to register
- Good for tightly-coupled systems where core controls the plugin list

## Isolation

### Why Isolate

- A buggy plugin shouldn't crash the core
- A malicious plugin shouldn't access secrets or the filesystem
- Plugins shouldn't interfere with each other

### Isolation Strategies

| Strategy | Security | Complexity | When to use |
|----------|---------|-----------|-------------|
| **Try/catch around plugin calls** | Low | Low | Trusted plugins, simple systems |
| **Subprocess isolation** | Medium | Medium | Untrusted plugins, CLI tools |
| **Sandboxed execution** | High | High | User-submitted plugins, web platforms |
| **Capability-based permissions** | High | High | Plugin marketplaces, enterprise systems |

**Capability model**: each plugin declares what it needs (filesystem read, network access, database write). Core grants only what's declared. Deny by default.

## Lifecycle

Every plugin goes through phases. Handle errors at each:

1. **Discovery**: Find the plugin. Fail gracefully if not found.
2. **Load**: Import the module. Catch import errors, log and skip.
3. **Initialize**: Call the plugin's setup/registration function. Catch and log errors, don't crash core.
7. **Run**: Execute plugin logic. Wrap in try/catch, don't let plugin errors propagate.
8. **Teardown**: Clean up resources (close connections, remove listeners). Call on shutdown.

```python
for plugin in plugins:
    try:
        plugin.initialize(config)
    except Exception as e:
        logger.error(f"Plugin {plugin.name} failed to initialize: {e}")
        # Skip this plugin, don't crash the app
```

## Testing Plugins

- **Test the plugin against the real host**, not a mock of the plugin API
- Create a test harness that loads the core + plugin together
- Test: registration, hook execution, error handling, teardown
- Test with the minimum and maximum supported core versions
- Integration test: install plugin, run core, verify plugin behavior

## Documentation for Plugin Authors

Plugin authors need more than API signatures:

- **Getting started guide**: create a minimal plugin in 5 minutes
- **Working examples**: one complete plugin for each extension point type
- **API reference**: every method, parameter, return type, and error case
- **Migration guide**: what changed between core versions, how to update
- **Debugging tips**: how to see logs, how to test locally, common errors

## Checklist

- [ ] Plugin pattern chosen (hooks, middleware, strategy, dynamic loading)
- [ ] Plugin API is versioned and documented
- [ ] Discovery mechanism defined (file scan, registry, built-in)
- [ ] Isolation strategy matches trust level (try/catch → sandbox)
- [ ] Lifecycle phases all handle errors without crashing core
- [ ] Plugin capabilities are explicitly declared and granted
- [ ] Tests run plugins against the real core, not mocks
- [ ] Documentation includes working examples, not just API signatures
