FROM lukemathwalker/cargo-chef:latest-rust-1.74.0 as chef
WORKDIR /app
RUN apt update && apt install lld clang -y

FROM chef as planner
COPY ../.. .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN apt-get install musl-tools clang llvm -y libssl-dev
RUN rustup target add aarch64-unknown-linux-musl
RUN cargo chef cook --release --recipe-path recipe.json
COPY ../.. .
RUN cargo build --release --bin zero2prod

FROM amazonlinux:2023 as runtime
WORKDIR /app

RUN yum update ca-certificates

COPY --from=builder /app/target/release/zero2prod zero2prod
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY configuration/base.yaml configuration/production.yaml configuration/
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.7.2 /lambda-adapter /opt/extensions/lambda-adapter
ENV APP_ENVIRONMENT=production

ENTRYPOINT ["./zero2prod"]

## Builder

#FROM public.ecr.aws/docker/library/rust:latest as BUILDER
#
#COPY ./ usr/src/zero2prod/
#
#WORKDIR /usr/src/zero2prod/
#
#RUN apt-get update && \
#    apt-get -y install sudo
#
## we need this to compile ring / open-ssl
#RUN apt-get install musl-tools clang llvm -y libssl-dev
#
## Install target platform (Cross-Compilation) --> Needed for Scratch (or Alpine)
#RUN rustup target add aarch64-unknown-linux-musl
#
## we use sqlx-data.json to validate the sqlx queries
#ARG SQLX_OFFLINE=true
#
## Build for Scratch (or Alpine)
#RUN cargo build --target aarch64-unknown-linux-musl --release
#
### Runtime
#
#FROM scratch AS RUNTIME
#
#COPY --from=BUILDER /usr/src/zero2prod/target/aarch64-unknown-linux-musl/release/zero2prod /
#COPY configuration/base.yaml configuration/production.yaml configuration/
#COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.7.1 /lambda-adapter /opt/extensions/lambda-adapter
#ENV APP_ENVIRONMENT=production
## Listen on the port
#EXPOSE 8080/tcp
#
## Run the App
#CMD ["./zero2prod"]

