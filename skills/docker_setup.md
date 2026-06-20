# Docker Setup — Dockerfile & Compose

## Basic Dockerfile

```dockerfile
FROM rust:1.81-slim AS build
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=build /app/target/release/app /app
CMD ["/app"]
```

## docker-compose.yml

```yaml
services:
  app:
    build: .
    ports: ["8080:8080"]
    environment:
      - DATABASE_URL=postgres://user:pass@db:5432/mydb
  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: pass
```

## Common commands

```bash
docker compose up -d          # start services
docker compose logs -f        # follow logs
docker compose down -v        # stop + remove volumes
docker compose exec app bash  # shell into container
```
