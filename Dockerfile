FROM rust:slim-bookworm AS builder

WORKDIR /usr/src/app

COPY Cargo.toml ./
COPY src ./src

RUN cargo build --release

# This contains glibc and ca-certificates, but no OS vulnerabilities
FROM gcr.io/distroless/cc-debian12

WORKDIR /app

# Copy the binary 
COPY --from=builder /usr/src/app/target/release/qrcode-generator ./

EXPOSE 3000

CMD ["./qrcode-generator"]