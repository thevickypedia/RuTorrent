# Build stage
FROM rust:1.95-alpine AS builder
RUN apk add --no-cache ca-certificates && update-ca-certificates
WORKDIR /app
COPY . .
RUN ls -ltrh && cargo build --release

# Runtime stage
FROM scratch
COPY --from=builder /app/target/release/rutorrent /rutorrent
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ENTRYPOINT ["/rutorrent"]
