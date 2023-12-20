#FROM lukemathwalker/cargo-chef:latest-rust-1.74.0 as chef
#WORKDIR /app
#RUN apt update && apt install lld clang -y
#
#FROM chef as planner
#COPY . .
#RUN cargo chef prepare --recipe-path recipe.json
#
#FROM chef as builder
#COPY --from=planner /app/recipe.json recipe.json
#RUN cargo chef cook --release --recipe-path recipe.json
#COPY . .
#ENV SQLX_OFFLINE true
#COPY migrations/ migrations/
#RUN cargo build --release --bin zero2prod
#
#FROM debian:bookworm-slim as runtime
#WORKDIR /app
#
#RUN apt-get update -y \
#    && apt-get install -y --no-install-recommends openssl ca-certificates \
#    && apt-get autoremove -y \
#    && apt-get clean -y \
#    && rm -rf /var/lib/apt/lists/*
#
#COPY --from=builder /app/target/release/zero2prod zero2prod
#COPY migrations/ migrations/
#COPY configuration/base.yaml configuration/production.yaml configuration/
#COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.7.1 /lambda-adapter /opt/extensions/lambda-adapter
#ENV APP_ENVIRONMENT=production
#
#ENTRYPOINT ["./zero2prod"]

## Builder

FROM public.ecr.aws/docker/library/rust:latest as BUILDER

COPY ./ usr/src/zero2prod/

WORKDIR /usr/src/zero2prod/

RUN apt-get update && \
    apt-get -y install sudo

# we need this to compile ring / open-ssl
RUN apt-get install musl-tools clang llvm -y

# Install target platform (Cross-Compilation) --> Needed for Scratch (or Alpine)
RUN rustup target add aarch64-unknown-linux-musl

# we use sqlx-data.json to validate the sqlx queries
ARG SQLX_OFFLINE=true

# Build for Scratch (or Alpine)
RUN cargo build --target aarch64-unknown-linux-musl --release

## Runtime

FROM scratch AS RUNTIME

COPY --from=BUILDER /usr/src/zero2prod/target/aarch64-unknown-linux-musl/release/zero2prod /
COPY configuration/base.yaml configuration/production.yaml configuration/
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.7.1 /lambda-adapter /opt/extensions/lambda-adapter
ENV APP_ENVIRONMENT=production
# Listen on the port
EXPOSE 8080/tcp

# Run the App
CMD ["./zero2prod"]

