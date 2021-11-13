FROM rust:1.56-alpine as builder
RUN apk add --no-cache musl-dev
WORKDIR /usr/src/watame
COPY . .
RUN cargo install --path .

FROM alpine:3.14
COPY --from=builder /usr/local/cargo/bin/watame /usr/local/bin/watame
COPY --from=builder /usr/src/watame/docker_run.sh /usr/local/bin/watame_run.sh
CMD ["watame_run.sh"]
