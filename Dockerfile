# BUILD CONTAINER

FROM rust:1.79 as build

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

RUN apt-get update && apt-get install -y libpq-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /Beelzebub
ADD . ./
RUN cargo build --bin beelzebub-server --release --verbose


# RUNTIME CONTAINER

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libpq5 && rm -rf /var/lib/apt/lists/*

COPY --from=build /Beelzebub/target/release/beelzebub-server .

EXPOSE 8080

CMD ["./beelzebub-server"]
