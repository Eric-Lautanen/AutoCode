---
name: api-integration
description: Use when integrating with any external REST, GraphQL, or WebSocket API - reading documentation, constructing requests, handling auth, parsing responses, and managing errors. Load when a task involves calling an external service or implementing a client for one.
---

# API Integration

## Overview

Integrating with an external API means your code now depends on something outside your control. The core principle: **be defensive at the boundary.** Assume the API will return unexpected shapes, will be slow, will fail, and will change. Code that handles all of these gracefully is code that doesn't wake you up at 3am.

## Reading API Documentation

Before writing any integration code, understand the API:

1. **Endpoints**: What URLs are available? What HTTP methods do they accept?
2. **Authentication**: How do you prove who you are? (API key, Bearer token, OAuth2)
3. **Request schemas**: What parameters are required vs. optional? What's the request body format?
4. **Response schemas**: What does a successful response look like? What does an error response look like?
5. **Rate limits**: How many requests can you make? What happens when you exceed them?
6. **Pagination**: How do you get the next page of results?

**Where to find docs:**
- Official documentation site (usually linked from the API root)
- OpenAPI/Swagger spec (often at `/docs`, `/swagger.json`, or `/openapi.json`)
- GraphQL introspection (run an introspection query to discover the schema)

## Auth Patterns

### API Keys
Simplest auth — a static string passed in a header or query parameter:
```
Authorization: Bearer sk_live_abc123
# or
?api_key=sk_live_abc123
```
- Store in environment variables, never hardcode
- Rotate regularly; support key rotation without downtime

### Bearer Tokens (OAuth2)
Token-based auth — obtain a token, then use it until it expires:
```
Authorization: Bearer eyJhbGciOiJIUzI1NiIs...
```
- Implement token refresh before the token expires
- Store tokens securely (not in localStorage for browser apps)

### OAuth2 Flows
- **Authorization Code**: For user-facing apps (redirect to provider, get code, exchange for token)
- **Client Credentials**: For server-to-server (exchange client ID + secret for token)
- **Always use a library** for OAuth2 — the spec has too many edge cases to implement correctly by hand

## Request Construction

### Headers
Common headers you'll need:
```
Content-Type: application/json        # What you're sending
Accept: application/json               # What you want back
Authorization: Bearer <token>          # Auth
User-Agent: MyApp/1.0                  # Identify your client
X-Request-ID: <uuid>                   # For tracing
```

### Query Parameters
- Use the API's documented parameter names exactly
- URL-encode special characters
- For filtering: `?status=active&created_after=2024-01-01`

### Request Body
- JSON for most modern APIs: `Content-Type: application/json`
- Form data for legacy APIs: `Content-Type: application/x-www-form-urlencoded`
- Multipart for file uploads: `Content-Type: multipart/form-data`

## Response Handling

### Status Codes
| Code | Meaning | What to do |
|------|---------|------------|
| 200 | OK | Parse the response body |
| 201 | Created | Parse the response body (new resource) |
| 204 | No Content | Success, no body to parse |
| 400 | Bad Request | Your request was malformed — fix it |
| 401 | Unauthorized | Auth failed — check credentials/refresh token |
| 403 | Forbidden | You're authenticated but not allowed — check permissions |
| 404 | Not Found | The resource doesn't exist — handle gracefully |
| 409 | Conflict | Duplicate or state conflict — handle per API docs |
| 422 | Unprocessable Entity | Valid format but invalid data — check validation errors |
| 429 | Too Many Requests | Rate limited — back off and retry |
| 500 | Server Error | Their problem — retry with backoff |

### Parsing JSON Responses
```python
# Always handle parsing errors
try:
    data = response.json()
except json.JSONDecodeError:
    # API returned non-JSON (HTML error page, empty body)
    handle_unexpected_response(response.text)
```

**Validate the shape before using it:**
- Check required fields exist: `"id" in data`
- Check types: `isinstance(data["count"], int)`
- Use schema validation (pydantic, Zod, serde) for complex responses

## Error Handling

### 4xx vs. 5xx
- **4xx (client errors)**: Your fault. Fix the request. Don't retry the same request — it will fail again.
- **5xx (server errors)**: Their fault. Retry with backoff.

### Retry Logic
```python
def call_api_with_retry(url, max_retries=3, base_delay=1.0):
    for attempt in range(max_retries):
        response = requests.get(url)
        if response.status_code < 500:
            return response  # Success or client error — don't retry
        if attempt < max_retries - 1:
            delay = base_delay * (2 ** attempt) + random.uniform(0, 1)  # jitter
            time.sleep(delay)
    return response  # Return the last failure
```

**Retry only on:**
- 5xx errors (server errors)
- 429 (rate limited) — respect Retry-After header
- Network errors (connection timeout, DNS failure)

**Never retry on:**
- 4xx errors (your request is wrong, retrying won't help)
- 401 (your credentials are bad, retrying won't fix them)

### Exponential Backoff
```
1st retry: wait ~1s
2nd retry: wait ~2s
3rd retry: wait ~4s
4th retry: wait ~8s
```
Always add jitter (random delay) to avoid thundering herd when multiple clients retry simultaneously.

## Rate Limiting

### Detecting Rate Limits
- **429 status code**: The standard signal
- **Rate limit headers**: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`
- **Retry-After header**: Seconds until you can retry

### Handling Rate Limits
1. Check `X-RateLimit-Remaining` before making requests if available
2. On 429, read `Retry-After` and wait that many seconds
3. If no `Retry-After`, use exponential backoff
4. For high-throughput: implement a token bucket or leaky bucket

## Testing API Integrations

### Record/Replay (VCR, Polly)
Record real API responses, replay them in tests:
- Fast, deterministic tests
- No network dependency
- Update recordings when the API changes

### Mock Servers (WireMock, Prism)
Run a local server that mimics the real API:
- Test error cases the real API doesn't easily produce
- Full control over responses
- Good for integration tests

### Real Sandbox Environments
- Use when available (Stripe, Twilio, etc. provide test environments)
- Most realistic but slowest and least deterministic
- Run in CI only for critical paths

See also: `error_handling_design` for error propagation patterns, `security_basics` for credential handling.
