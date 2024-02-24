FROM docker.io/rust:1 as build
WORKDIR /build
COPY . /build
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /build/target/release/sd-notify-adapter .
CMD ["./sd-notify-adapter"]
