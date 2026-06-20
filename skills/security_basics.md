---
name: security-basics
description: Use when implementing any feature that handles user input, authentication, file paths, credentials, external data, or network requests. Covers the most common security mistakes in application code and how to avoid them. Load before implementing auth, file handling, user input processing, or any external-facing interface.
---

# Security Basics

## Overview

Security in application code is about closing the most common attack vectors. The core principle: **never trust input from outside your code boundary.** Every piece of data from a user, an API, a file, or the network is potentially malicious. Validate at the boundary, parameterize everything, and expose the minimum necessary.

## Input Validation

### Validate at the Boundary
Validate input as soon as it enters your system — at the API handler, the controller, the CLI argument parser. Not in business logic deeper in the code.

```python
# BAD: validate deep in the codebase
def save_user(name):
    if len(name) > 255:  # validation too late, data already flowed through 3 layers
        raise ValueError("Name too long")
    db.save(name)

# GOOD: validate at the boundary
@app.post("/users")
def create_user(request: CreateUserRequest):
    validate(request)  # fails fast, before any processing
    service.create_user(request)
```

### Whitelist Over Blacklist
- **Whitelist**: Allow only known-good values. `"status" in ["active", "inactive", "suspended"]`
- **Blacklist**: Block known-bad values. `"status" not in ["admin", "root"]` — you'll miss something.

**Always prefer whitelists.** It's impossible to enumerate all bad inputs; it's feasible to enumerate all valid ones.

## Injection Attacks

### SQL Injection
```python
# NEVER DO THIS
cursor.execute(f"SELECT * FROM users WHERE email = '{email}'")

# ALWAYS DO THIS
cursor.execute("SELECT * FROM users WHERE email = $1", (email,))
```

Even if you think the input is safe, parameterize. The one time you skip it will be the time someone exploits it.

### Shell Injection
```python
# NEVER DO THIS
os.system(f"convert {filename} output.png")

# ALWAYS DO THIS
subprocess.run(["convert", filename, "output.png"], shell=False)
```

**Rule:** Never pass user input to a shell. Use argument arrays, not string interpolation.

### Path Traversal
```python
# NEVER DO THIS
with open(f"/data/{user_filename}") as f:  # user_filename = "../../etc/passwd"
    data = f.read()

# ALWAYS DO THIS
base = Path("/data")
resolved = (base / user_filename).resolve()
if not str(resolved).startswith(str(base.resolve())):
    raise ValueError("Invalid path")
with open(resolved) as f:
    data = f.read()
```

## Authentication

### Never Roll Your Own Crypto
- Use established libraries: bcrypt/argon2 for passwords, JWT libraries for tokens, OAuth2 libraries for flows
- Don't implement encryption algorithms, hash functions, or key exchange protocols yourself
- Even experts get crypto wrong. You are not an exception.

### Password Handling
- **Hash with bcrypt or argon2.** Never MD5, SHA1, SHA256 for passwords — they're too fast.
- **Salt automatically.** bcrypt and argon2 handle salting internally.
- **Never store plaintext passwords.** Not in logs, not in debug output, not in error messages.

## Secrets

### Never Hardcode
```python
# NEVER
API_KEY = "sk_live_abc123"

# ALWAYS
API_KEY = os.environ["API_KEY"]
```

### Never Log
```python
# NEVER
logger.info(f"Connecting with token: {token}")

# ALWAYS
logger.info("Connecting to API")  # token is in env, not in logs
```

### Never Put in URLs or Error Messages
- URLs are logged by proxies, CDNs, and browsers
- Error messages may be shown to users or stored in monitoring systems
- Use a reference ID instead: "Authentication failed. Reference: abc-123"

## File Handling

- **Validate paths**: Resolve and check that the path is within the expected directory
- **Restrict to expected directories**: Never let users specify arbitrary paths
- **Check file types**: Validate by content (magic bytes), not by extension (easily spoofed)
- **Limit file size**: Reject files larger than your expected maximum
- **Don't execute user-uploaded files**: Store uploads outside the web root

## Dependencies

- **Audit regularly**: `npm audit`, `cargo audit`, `pip-audit`
- **Update promptly**: When a vulnerability is disclosed, update the dependency
- **Minimize dependencies**: Fewer dependencies = smaller attack surface
- **Check licenses**: Ensure compatibility with your project's license
- **Supply chain**: Prefer well-maintained, widely-used packages with multiple maintainers

## Least Privilege

- **Request only the permissions you need.** Don't ask for admin access when read-only suffices.
- **Database users**: Create separate users for different operations (read-only for reporting, read-write for application)
- **API tokens**: Use the narrowest scope possible. A token that can only read one resource is better than one that can read everything.
- **File permissions**: Don't run services as root. Use the minimum user privileges needed.

## HTTPS

- **Always use HTTPS for external communication.** No exceptions.
- **Validate certificates.** Don't disable certificate verification (`verify=False` in requests, `NODE_TLS_REJECT_UNAUTHORIZED=0`).
- **Use TLS 1.2+ minimum.** TLS 1.0 and 1.1 have known vulnerabilities.
- **HSTS**: Set the `Strict-Transport-Security` header to prevent downgrade attacks.

## Common Security Checklist

Before deploying any external-facing code:

- [ ] All user input is validated at the boundary
- [ ] All database queries use parameterized statements
- [ ] All shell commands use argument arrays, not string interpolation
- [ ] File paths are validated and restricted to expected directories
- [ ] No secrets are hardcoded, logged, or exposed in error messages
- [ ] Authentication uses established libraries, not custom implementations
- [ ] Passwords are hashed with bcrypt or argon2
- [ ] HTTPS is used for all external communication
- [ ] Dependencies are audited for known vulnerabilities
- [ ] The service runs with minimum necessary permissions

## Anti-Patterns

- **"It's only internal."** Internal services get breached too. Apply the same standards.
- **Disabling certificate verification.** `verify=False` is never the right answer. Fix the certificate.
- **Rolling your own auth.** Use a library. Auth has too many edge cases.
- **Trusting client-side validation.** Server must validate everything. Client validation is UX, not security.
- **Storing secrets in source code.** Even in comments. Even in test files. Git history is forever.
