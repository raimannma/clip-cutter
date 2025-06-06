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
    openssl \
    build-essential \
    pkg-config \
    ffmpeg \
    pipx \
    && rm -rf /var/lib/apt/lists/*

ENV PATH=/root/.local/bin:$PATH
RUN pipx ensurepath && pipx install twitch-dl && pipx install yt-dlp && pipx install gunicorn && pipx install flask

ENV LD_LIBRARY_PATH=/usr/local/lib
COPY --from=builder /app/target/release/clip-cutter /usr/local/bin/clip-cutter
COPY --from=builder /app/target/release/libonnxruntime.so.1.16.0 /usr/local/lib/libonnxruntime.so.1.16.0
