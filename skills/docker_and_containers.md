---
name: docker-and-containers
description: Use when writing Dockerfiles, docker-compose files, or working with containerized applications - building images, managing services, debugging container issues, or optimizing image size. Load when any task involves Docker, containers, or containerized deployment.
---

# Docker and Containers

## Overview

Containers package your application with its dependencies into a reproducible unit. The core principle: **containers should be deterministic, minimal, and stateless.** A container built today should produce the same result as one built tomorrow. A container should contain only what's needed to run. And a container should be replaceable — if it dies, another one takes its place.

## Dockerfile Best Practices

### Layer Ordering for Cache Efficiency
Docker builds images layer by layer, caching each layer. Order from least to most frequently changing:

```dockerfile
# 1. Base image (changes rarely)
FROM node:20-slim

# 2. System dependencies (changes rarely)
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 3. Install application dependencies (changes when deps change)
COPY package.json package-lock.json ./
RUN npm ci --only=production

# 4. Copy application code (changes most frequently)
COPY . .

# 5. Runtime configuration
EXPOSE 3000
CMD ["node", "server.js"]
```

**Why order matters:** If you copy all source code before `npm install`, every code change invalidates the dependency cache. By copying `package.json` first, dependency installation is cached unless `package.json` changes.

### Multi-Stage Builds
Use a build stage and a runtime stage to keep the final image small:

```dockerfile
# Build stage
FROM node:20 AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

# Runtime stage — only what's needed to run
FROM node:20-slim
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/package.json ./
CMD ["node", "dist/server.js"]
```

**Result:** The final image doesn't include the build tools, source code, or dev dependencies.

## Base Image Selection

| Image | Size | When to use |
|-------|------|-------------|
| `node:20` | ~1GB | Need full OS tools, debugging |
| `node:20-slim` | ~250MB | Production — recommended default |
| `node:20-alpine` | ~180MB | Minimal size, but musl libc may cause issues |
| `distroless` | ~50MB | Maximum security, no shell for debugging |

**Rules:**
- Use official images from Docker Hub
- Pin the version: `node:20.10-slim` not `node:latest`
- Prefer `-slim` over full images for production
- Use Alpine only if you test with musl libc (some Node native modules don't work)

## .dockerignore

Create a `.dockerignore` file to exclude files from the build context:

```
node_modules
.git
.env
dist
*.md
Dockerfile
docker-compose*.yml
```

**Why it matters:**
- Smaller build context = faster builds
- Excluding `.git` prevents leaking git history into the image
- Excluding `node_modules` prevents stale local deps from being copied (let `npm ci` install fresh ones)

## docker-compose

### Service Definitions
```yaml
version: "3.8"
services:
  app:
    build: .
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgres://user:pass@db:5432/myapp
    depends_on:
      db:
        condition: service_healthy
    volumes:
      - ./src:/app/src  # Dev: live reload

  db:
    image: postgres:16
    environment:
      POSTGRES_USER: user
      POSTGRES_PASSWORD: pass
      POSTGRES_DB: myapp
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U user"]
      interval: 5s
      retries: 5

volumes:
  pgdata:
```

### Key Patterns
- **depends_on with healthcheck**: Don't start the app until the database is ready
- **Named volumes**: For persistent data (database storage)
- **Bind mounts**: For development (live code reload)
- **Environment variables**: Configure services without rebuilding

## Common Commands

| Command | Purpose |
|---------|---------|
| `docker build -t myapp .` | Build an image |
| `docker run -p 3000:3000 myapp` | Run a container |
| `docker exec -it <container> sh` | Shell into a running container |
| `docker logs <container>` | View container logs |
| `docker ps` | List running containers |
| `docker compose up` | Start all services |
| `docker compose down` | Stop and remove all services |
| `docker compose logs -f app` | Follow logs for one service |
| `docker inspect <container>` | Detailed container info |

## Debugging Containers

### Exec Into a Running Container
```bash
docker exec -it myapp sh          # Shell into the container
docker exec -it myapp bash        # If bash is available
docker exec -it myapp /bin/sh     # Alpine uses /bin/sh
```

### Read Logs
```bash
docker logs myapp                 # All logs
docker logs --tail 100 myapp      # Last 100 lines
docker logs -f myapp              # Follow logs
```

### Inspect Mounts and Environment
```bash
docker inspect myapp              # Full config
docker inspect --format='{{range .Mounts}}{{.Source}} -> {{.Destination}}{{end}}' myapp
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' myapp
```

### Common Issues
- **Container exits immediately**: Check `docker logs <container>` for the error
- **Can't connect to service**: Check the network, use service names (not localhost) in compose
- **File not found in container**: Check the COPY paths and working directory
- **Permission denied**: Check USER directive, file ownership, and volume mount permissions

## Image Size Reduction

1. **Multi-stage builds**: Build in one stage, copy only artifacts to runtime stage
2. **Remove build dependencies**: `npm ci --only=production` (no devDependencies)
3. **Use slim/alpine base images**: Smaller footprint
4. **Clean package manager cache**: `rm -rf /var/lib/apt/lists/*` in same RUN layer
5. **Combine RUN commands**: Each RUN creates a layer; combine related operations
6. **Don't install debug tools in production**: No `curl`, `vim`, `htop` in prod images

## Environment Parity

Make containers behave consistently across dev/CI/prod:

- **Same image everywhere.** Don't use different base images for dev and prod.
- **Configuration via environment variables.** Same image, different `.env` files.
- **Don't rely on container IP addresses.** Use service names in compose, DNS in orchestration.
- **Stateless containers.** Don't store data in the container filesystem — use volumes or external storage.

## Anti-Patterns

- **Running as root.** Add `USER node` or equivalent to your Dockerfile.
- **Not using .dockerignore.** Bloated build context and potential secret leaks.
- **Copying everything before installing deps.** Kills build cache efficiency.
- **Using `latest` tag for base images.** Pin versions for reproducibility.
- **Storing data in the container.** Containers are ephemeral — use volumes.
- **One giant Dockerfile for multiple services.** Each service gets its own Dockerfile.
- **Not health-checking dependencies.** Your app will start before the database is ready.
