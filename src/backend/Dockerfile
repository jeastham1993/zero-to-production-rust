FROM lukemathwalker/cargo-chef:latest-rust-1.75.0 as chef
WORKDIR /app
RUN apt update && apt install lld clang -y

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin backend

FROM amazonlinux:2023 as runtime
WORKDIR /app

RUN yum update ca-certificates

WORKDIR /app

COPY --from=builder /app/target/release/backend backend
COPY configuration/base.yaml configuration/production.yaml configuration/
ENV APP_ENVIRONMENT=production

ENTRYPOINT ["./backend"]