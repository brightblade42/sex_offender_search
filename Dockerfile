FROM ekidd/rust-musl-builder:latest as builder
ADD . ./
RUN sudo chown -R rust:rust /home/rust
RUN cargo build --release
FROM alpine:latest
WORKDIR /usr/local/app
RUN mkdir docs
RUN apk --no-cache add ca-certificates
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/sex_offender_search /usr/local/app/sex_offender_search
COPY ./docs/sex_offender_search.html docs/sex_offender_search.html
ENV SXOFF_DB=/usr/local/data/sexoffenders.sqlite
ENV AUTH_DB=/usr/local/data/auth.db
EXPOSE 80 8080 8090
CMD ["/usr/local/app/sex_offender_search"]
