FROM ekidd/rust-musl-builder as builder
ADD . ./
RUN sudo chown -R rust:rust /home/rust
RUN cargo build --release

FROM alpine:latest
RUN apk --no-cache add ca-certificates

COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/sex_offender_search /usr/local/bin/sex_offender_search

ENV SQL_PATH=/usr/local/data/sexoffenders.sqlite
EXPOSE 8000
CMD ["/usr/local/bin/sex_offender_search"]




#the command to run. 
#RUN ["/usr/app/sex_offender_search"]
