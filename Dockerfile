FROM alpine:latest
WORKDIR /app
COPY bin /app/bin
ENTRYPOINT ["/app/bin"]
