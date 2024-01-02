FROM rust as builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    build-essential \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release

FROM debian

WORKDIR /app

RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

COPY ./twitch-dl /usr/local/bin/twitch-dl
COPY ./model.onnx /app/model.onnx

ENV LD_LIBRARY_PATH=/usr/local/lib
COPY --from=builder /app/target/release/clip-cutter /usr/local/bin/clip-cutter
COPY --from=builder /app/target/release/libonnxruntime.so.1.16.0 /usr/local/lib/libonnxruntime.so.1.16.0
