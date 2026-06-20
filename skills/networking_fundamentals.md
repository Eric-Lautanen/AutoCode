---
name: networking-fundamentals
description: Use when debugging connection issues, implementing network clients, understanding latency, working with DNS, TLS, proxies, or any task that requires understanding how data actually moves between machines. Load when a task involves network errors, connection timeouts, certificate issues, or building anything that communicates over a network.
---

# Networking Fundamentals

## Overview

Every networked application depends on a stack of protocols: DNS to find the host, TCP to establish a reliable connection, TLS to secure it, and HTTP (or another application protocol) to exchange data. When something breaks, you need to know which layer failed to fix it efficiently. This skill covers the networking concepts that developers encounter most often, with practical debugging guidance for each layer.

## TCP: The Transport Layer

### Connection Lifecycle

```
Client                          Server
  |--- SYN (seq=100) ---------->|    1. Client initiates
  |<-- SYN-ACK (seq=300, -------|    2. Server acknowledges
  |    ack=101)                 |    3. Client confirms
  |--- ACK (ack=301) ---------->|    
  |                             |    [Data flows both ways]
  |--- FIN -------------------->|    4. Client closes
  |<-- FIN-ACK -----------------|    5. Server acknowledges
  |--- ACK -------------------->|    6. Client confirms
```

### Why Connections Fail

| Symptom | Likely cause | Debug |
|---------|-------------|-------|
| "Connection refused" | Server not listening on that port | Check server is running, check port |
| "Connection timed out" | Firewall blocking, wrong IP, server down | `telnet host port`, `nc -zv host port` |
| "Connection reset" | Server crashed, or rejected after connect | Check server logs |
| "Address not reachable" | DNS failure or wrong network | `ping` the host, check DNS |

### TIME_WAIT

After a connection closes, the side that initiated the close keeps the socket in TIME_WAIT for ~60 seconds. This is normal — it prevents delayed packets from a previous connection being misinterpreted.

**Problem**: Many short-lived connections (e.g., HTTP/1.1 without keep-alive) exhaust available ports.
**Fix**: Use connection pooling, enable keep-alive, or switch to HTTP/2 (multiplexes over one connection).

## DNS

### Resolution Chain

```
Browser/App
  → Local cache (browser, OS)
  → Stub resolver (OS)
  → Recursive resolver (ISP or 8.8.8.8 / 1.1.1.1)
  → Root nameserver (.)
  → TLD nameserver (.com)
  → Authoritative nameserver (example.com)
```

### Common Record Types

| Type | Purpose | Example |
|------|---------|---------|
| A | IPv4 address | `example.com → 93.184.216.34` |
| AAAA | IPv6 address | `example.com → 2606:2800:220:1:...` |
| CNAME | Alias to another domain | `www.example.com → example.com` |
| MX | Mail server | `example.com → mail.example.com` |
| TXT | Arbitrary text (SPF, DKIM, verification) | `"v=spf1 include:..."` |
| NS | Authoritative nameserver | `example.com → ns1.dnsprovider.com` |

### Propagation Delays

- DNS changes are not instant. TTL (Time To Live) determines how long resolvers cache the old value.
- Typical TTLs: 300s (5 min) for frequently changed records, 86400s (1 day) for stable records.
- **When changing DNS**: Lower the TTL 24 hours before the change, make the change, then raise it again.
- **When debugging**: Flush your local cache (`ipconfig /flushdns` on Windows, `sudo systemd-resolve --flush-caches` on Linux) and use `dig` or `nslookup` to check what the authoritative server returns.

## HTTP/1.1 vs HTTP/2 vs HTTP/3

| Feature | HTTP/1.1 | HTTP/2 | HTTP/3 |
|---------|----------|--------|--------|
| Transport | TCP | TCP | QUIC (UDP) |
| Multiplexing | No (one request per connection) | Yes (multiple streams per connection) | Yes |
| Head-of-line blocking | TCP level | TCP level (streams don't block each other, but packet loss blocks all streams) | No (QUIC streams are independent) |
| Connection setup | 1 RTT + TLS | 1 RTT + TLS | 0-1 RTT (TLS built into QUIC) |
| Server push | No | Yes (rarely used) | No |

**When it matters**:
- HTTP/2 helps when you make many concurrent requests to the same host (web apps with many assets)
- HTTP/3 helps on high-latency or lossy networks (mobile, cross-continental)
- HTTP/1.1 is fine for simple APIs with few concurrent requests

## TLS

### Handshake Overview

```
Client                          Server
  |--- ClientHello ------------>|  Supported cipher suites, TLS version
  |<-- ServerHello + Certificate |  Chosen cipher, server certificate
  |--- Key Exchange ------------>|  Client generates session key, encrypts with server's public key
  |<-- Finished -----------------|  Both derive session keys
  |--- Finished ---------------->|  Encrypted data flows
```

### Common TLS Errors

| Error | Cause | Fix |
|-------|-------|-----|
| "Certificate has expired" | Server cert past its validity date | Renew the certificate |
| "Certificate is not trusted" | Cert not signed by a known CA | Use a trusted CA (Let's Encrypt), or add to trust store |
| "Certificate name mismatch" | Cert's CN/SAN doesn't match the hostname | Use the correct hostname, or get a cert with the right name |
| "Self-signed certificate" | Cert not signed by a CA | Add to trust store for dev; never for production |
| "Handshake failure" | No compatible cipher suite or TLS version | Update client or server to support modern TLS 1.2+ |

### Debugging TLS

```bash
# Check certificate details
openssl s_client -connect example.com:443 -showcerts

# Check certificate expiration
echo | openssl s_client -connect example.com:443 2>/dev/null | openssl x509 -noout -dates

# Check supported TLS versions
nmap --script ssl-enum-ciphers -p 443 example.com
```

## Ports and Firewalls

### Well-Known Ports

| Port | Protocol | Common use |
|------|----------|-----------|
| 22 | SSH | Remote shell |
| 53 | DNS | Domain name resolution |
| 80 | HTTP | Unencrypted web |
| 443 | HTTPS | Encrypted web |
| 3306 | MySQL | Database |
| 5432 | PostgreSQL | Database |
| 6379 | Redis | Cache/message broker |
| 8080 | HTTP alt | Dev servers, proxies |

### Diagnosing Blocked Connections

```bash
# Test if a port is reachable
telnet host 443
nc -zv host 443

# On Windows
Test-NetConnection -ComputerName host -Port 443

# Check what's listening on a port
netstat -tlnp    # Linux
netstat -an      # Windows (findstr for port)

# Traceroute to find where packets stop
traceroute host   # Linux/Mac
tracert host      # Windows
```

## Proxies and Load Balancers

Proxies sit between client and server. They affect:

- **Headers**: Proxies add `X-Forwarded-For` (original client IP), `X-Forwarded-Proto` (original scheme)
- **IP addresses**: Your server sees the proxy's IP, not the client's. Use `X-Forwarded-For` but validate it (first untrusted proxy sets it)
- **TLS**: TLS termination at the proxy means the proxy-to-server connection might be unencrypted. Use mTLS or restrict network access.
- **WebSockets**: Proxies must be configured to allow WebSocket upgrade (set `Connection: upgrade` and `Upgrade: websocket`)

## Latency vs. Throughput

- **Latency**: Time for one request to complete (round-trip time). Affected by distance, network hops, server processing time.
- **Throughput**: How much data can be transferred per second. Affected by bandwidth, connection count, protocol efficiency.

**They are independent**: A satellite link has high latency (~500ms) but can have high throughput (Mbps). A local connection has low latency (<1ms) but may have low throughput if the server is slow.

**Optimization**:
- Reduce latency: fewer round trips (batch requests, use HTTP/2, cache, CDN)
- Increase throughput: parallel connections, compression, larger payloads per request

## Debugging Tools Quick Reference

| Tool | What it does |
|------|-------------|
| `curl -v` | Verbose HTTP request with headers |
| `dig` / `nslookup` | DNS lookup |
| `ping` | Test reachability and latency |
| `traceroute` / `tracert` | Show network path to host |
| `netstat` / `ss` | Show active connections and listening ports |
| `openssl s_client` | Test TLS connections and certificates |
| `tcpdump` / `Wireshark` | Capture and analyze network packets |
| `nc` (netcat) | Test TCP/UDP connectivity |

## Checklist

- [ ] Identified which layer the problem is at (DNS, TCP, TLS, HTTP)
- [ ] Used the right tool for the layer (dig for DNS, telnet/nc for TCP, curl for HTTP)
- [ ] Checked DNS resolution before assuming a connection issue
- [ ] Verified TLS certificate validity and hostname match
- [ ] Checked firewall rules if connection is refused or times out
- [ ] Considered proxy effects (X-Forwarded-For, TLS termination)
- [ ] Distinguished latency from throughput when optimizing
