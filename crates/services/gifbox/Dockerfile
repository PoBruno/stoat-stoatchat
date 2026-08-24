# Declarado antes do primeiro FROM porque um ARG usado em FROM precisa
# estar no escopo global. Sem isto o --build-arg BASE_IMAGE do CI e
# ignorado em silencio e a imagem sai com o binario do upstream.
ARG BASE_IMAGE=ghcr.io/stoatchat/base:latest

# Build Stage
FROM ${BASE_IMAGE} AS builder
# Bundle Stage
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /home/rust/src/target/release/revolt-gifbox ./

EXPOSE 14706
USER nonroot
CMD ["./revolt-gifbox"]
