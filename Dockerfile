FROM mirror.gcr.io/rust:1 AS builder
WORKDIR /src/app
COPY . /src/app
RUN RUSTFLAGS="-C target-cpu=native" cargo build --profile release-optimized

FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /src/app/target/release-optimized/exchange_api ./
USER nonroot
ENTRYPOINT ["./exchange_api"]
