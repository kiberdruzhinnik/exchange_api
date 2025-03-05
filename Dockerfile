FROM mirror.gcr.io/rust:1 as builder
WORKDIR /src/app
COPY . /src/app
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /src/app/target/release/exchange_api ./
ENTRYPOINT ["./exchange_api"]
