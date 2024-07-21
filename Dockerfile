# BUILD CONTAINER

FROM rust:1.79 as build

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

RUN apt-get update && apt-get install -y libpq-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /Beelzebub

# Build dependencies separately for layer caching.
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./client/Cargo.toml ./client/Cargo.toml
COPY ./server/Cargo.toml ./server/Cargo.toml
COPY ./shared/Cargo.toml ./shared/Cargo.toml
RUN mkdir -p client/src server/src shared/src && \
    echo "fn main() {}" > client/src/main.rs && \
    echo "fn main() {}" > server/src/main.rs && \
    touch shared/src/lib.rs && \
    cargo build --bin beelzebub-server --release --verbose

# Clean the temporary project.
RUN rm client/src/*.rs server/src/*.rs shared/src/*.rs ./target/release/deps/beelzebub* ./target/release/deps/libshared*

ADD . ./
RUN cargo build --bin beelzebub-server --release --verbose


# RUNTIME CONTAINER

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libpq5 && rm -rf /var/lib/apt/lists/*

COPY --from=build /Beelzebub/target/release/beelzebub-server .

EXPOSE 8080

CMD ["./beelzebub-server"]
