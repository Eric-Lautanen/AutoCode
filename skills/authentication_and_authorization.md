---
name: authentication-and-authorization
description: Use when implementing login, session management, token handling, role-based access, or any system that controls who can do what. Load when a task involves auth flows, JWT tokens, sessions, permissions, or access control logic.
---

# Authentication and Authorization

## Overview

Auth is the most security-sensitive part of any application. The core principle: **never implement your own cryptographic primitives — use established libraries for every auth operation.** A custom auth system is a security vulnerability waiting to happen. This skill covers the patterns, not the crypto — for crypto details, see `encryption_and_hashing`.

## AuthN vs. AuthZ

- **Authentication (AuthN)**: Who are you? Verifying identity (login, token validation)
- **Authorization (AuthZ)**: What can you do? Checking permissions (role checks, resource ownership)

**Always do both, in order.** Authentication first, then authorization. A user who isn't authenticated can't be authorized.

## Password Handling

### Hashing
- **Use bcrypt or argon2.** These are purpose-built for passwords — they're slow, which is the point.
- **Never use MD5, SHA1, SHA256** for passwords — they're too fast, making brute-force attacks trivial.
- **Salting is automatic** with bcrypt and argon2 — don't salt manually.

```python
# GOOD — bcrypt with automatic salting
import bcrypt
hashed = bcrypt.hashpw(password.encode(), bcrypt.gensalt(rounds=12))
bcrypt.checkpw(password.encode(), hashed)  # Verify

# NEVER
import hashlib
hashed = hashlib.sha256(password.encode()).hexdigest()  # Way too fast
```

### Password Requirements
- Minimum length: 8+ characters (longer is better)
- Don't enforce arbitrary complexity rules (must contain uppercase, number, symbol) — they reduce entropy
- Check against common password lists (haveibeenpwned API)
- Rate limit login attempts (see below)

## Session-Based Auth

### How It Works
1. User logs in with credentials
2. Server creates a session, stores it server-side, sends session ID in a cookie
3. Browser sends cookie with every request
4. Server looks up session by ID

### Cookie Security Flags
```
Set-Cookie: session_id=abc123;
  HttpOnly;      # JavaScript can't access this cookie (XSS protection)
  Secure;        # Only sent over HTTPS
  SameSite=Lax;  # CSRF protection — cookie not sent on cross-site requests
  Path=/;
  Max-Age=86400  # 24 hours
```

**Always set all three:** `HttpOnly`, `Secure`, `SameSite=Lax` (or `Strict` for highest security).

### Session Storage
- **In-memory**: Fast, lost on restart, doesn't scale across servers
- **Redis/Memcached**: Fast, shared across servers, survives restarts
- **Database**: Durable, slower, good for audit requirements

## JWT (JSON Web Tokens)

### Structure
```
Header.Payload.Signature
```
- **Header**: Algorithm and type (`{"alg": "HS256", "typ": "JWT"}`)
- **Payload**: Claims (`{"sub": "123", "exp": 1705312200}`)
- **Signature**: HMAC or RSA signature over header + payload

### Key Rules
- **JWT is signed, not encrypted.** Anyone can read the payload. Never put secrets in a JWT.
- **Always verify the signature.** If you don't verify, the token could be forged.
- **Always check expiry.** `exp` claim must be in the future.
- **Use a short expiry** (15 minutes for access tokens). Use refresh tokens for longer sessions.
- **Don't store sensitive data in the payload.** User ID and roles are fine. Email addresses and PII are not.

### Refresh Tokens
```
Access token: short-lived (15 min), used for API calls
Refresh token: long-lived (7 days), used only to get new access tokens
```
- Store refresh tokens in `HttpOnly` cookies, not localStorage
- Refresh tokens should be one-time use — issue a new refresh token with each access token refresh
- Revoke refresh tokens on logout and password change

### JWT vs. Sessions
| Aspect | JWT | Sessions |
|--------|-----|----------|
| Server state | Stateless | Stateful |
| Revocation | Hard (token valid until expiry) | Easy (delete session) |
| Scaling | Easy (no server storage) | Needs shared session store |
| Size | Large (token in every request) | Small (cookie with session ID) |

**Use JWT when:** You need stateless auth, microservices, or cross-domain SSO.
**Use sessions when:** You need easy revocation, small cookie size, or simple server-side auth.

## OAuth2 / OIDC

### Common Flows

**Authorization Code (for user-facing apps):**
1. Redirect user to provider's login page
2. User authenticates, provider redirects back with an auth code
3. Server exchanges auth code for access token
4. Use access token for API calls

**Client Credentials (for server-to-server):**
1. Server sends client ID + secret to token endpoint
2. Provider returns an access token
3. Use token for API calls

**Always use a library.** OAuth2 has too many edge cases (PKCE, token revocation, refresh rotation) to implement correctly by hand.

### When to Use OAuth2
- "Sign in with Google/GitHub/Apple" — delegate auth to a provider
- Your API is consumed by third-party applications
- You need cross-service SSO

## RBAC Basics

### Roles, Permissions, and Checks
```python
# Define roles with permissions
ROLES = {
    "admin": ["users:read", "users:write", "orders:read", "orders:write"],
    "manager": ["orders:read", "orders:write"],
    "viewer": ["orders:read"],
}

# Check at the right layer — at the API boundary, not in business logic
@require_permission("orders:write")
def update_order(order_id, data):
    ...
```

### Checking at the Right Layer
- **API layer**: Check that the user has the required permission for this endpoint
- **Business logic**: Should not contain auth checks — it should receive already-authorized requests
- **Data layer**: Row-level security if needed (user can only see their own data)

## Common Mistakes

- **Authorization checked only in the UI.** The "admin" button is hidden, but the API endpoint has no check. Always enforce auth on the server.
- **Tokens without expiry.** A JWT that never expires is a permanent credential if leaked.
- **Broad OAuth scopes.** Request only the scopes you need. `read:all` is a security risk.
- **Not invalidating sessions on logout.** The session should be destroyed server-side, not just the cookie cleared client-side.
- **Storing tokens in localStorage.** Vulnerable to XSS. Use HttpOnly cookies.

## Logout

### What to Do on Logout
1. **Invalidate the session** (delete from server store) or **revoke the refresh token**
2. **Clear cookies** (set expiry to past)
3. **For JWT**: Add the token to a blocklist (if you need immediate revocation) or rely on short expiry
4. **Redirect to login page**

### Revoking Refresh Tokens
- Store refresh tokens in a database
- On logout, delete the refresh token from the database
- On password change, delete all refresh tokens for that user

## Anti-Patterns

- **Rolling your own auth.** Use established libraries. Auth has too many edge cases.
- **Storing passwords in plain text or fast hashes.** bcrypt or argon2 only.
- **Not using HttpOnly/Secure/SameSite on auth cookies.** XSS and CSRF attacks exploit this.
- **Long-lived JWTs with no revocation.** If a token is compromised, it's valid until expiry.
- **Checking auth only in the frontend.** Server must enforce every access check.
