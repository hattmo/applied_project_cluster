# Multi-stage build for NPC UI (Rust backend + React frontend)

# =============================================================================
# Stage 1: Build Frontend (React)
# =============================================================================
FROM node:20-alpine AS frontend-builder

WORKDIR /app/frontend

COPY frontend/package.json ./
RUN npm install

COPY frontend/ ./
RUN npm run build

# =============================================================================
# Stage 2: Build Backend (Rust)
# =============================================================================
FROM rust:1.85-alpine AS backend-builder

# Install dependencies
RUN apk add --no-cache musl-dev

WORKDIR /app/backend

COPY backend/Cargo.toml ./
COPY backend/src/ ./src/

# Build release binary
RUN cargo build --release

# =============================================================================
# Stage 3: Runtime Image
# =============================================================================
FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache ca-certificates libgcc

# Create non-root user
RUN addgroup -g 1000 appgroup && \
    adduser -u 1000 -G appgroup -s /bin/sh -D appuser

WORKDIR /app

# Copy backend binary
COPY --from=backend-builder /app/backend/target/release/npc-ui-backend /app/backend

# Copy frontend build to static files directory
COPY --from=frontend-builder /app/frontend/dist /app/frontend/static

# Set ownership
RUN chown -R appuser:appgroup /app

USER appuser

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:3000/health || exit 1

# Run backend
CMD ["/app/backend"]
