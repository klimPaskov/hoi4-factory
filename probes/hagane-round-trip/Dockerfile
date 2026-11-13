FROM golang:1.25-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends jq curl unzip && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY hs/ /src/
RUN mkdir -p web/dist && printf '<!doctype html><title>placeholder</title>' > web/dist/index.html \
 && CGO_ENABLED=0 go build -p 4 -o /usr/local/bin/haganeworks ./cmd/haganeworks \
 && CGO_ENABLED=0 go build -p 4 -o /usr/local/bin/hw ./cmd/hw \
 && rm -rf /root/.cache/go-build
