FROM ekidd/rust-musl-builder as builder
ADD . ./
RUN sudo chown -R rust:rust /home/rust
RUN cargo build --release
FROM alpine:latest
WORKDIR /usr/local/app
RUN mkdir docs
RUN apk --no-cache add ca-certificates
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/sex_offender_search /usr/local/app/sex_offender_search
COPY ./docs/sex_offender_search.html docs/sex_offender_search.html
ENV SQL_PATH=/usr/local/data/sexoffenders.sqlite
EXPOSE 80 8080 8090
CMD ["/usr/local/app/sex_offender_search"]
