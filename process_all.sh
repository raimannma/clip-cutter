#!/usr/bin/bash

source /root/.bashrc || true

trap stop SIGINT

clean() {
  echo "Cleaning..."
  mkdir -p clips;
  rm -rf clips/*;

  mkdir -p matches;
  rm -rf matches/*;
}

stop() {
  rclone sync -P state/ Nextcloud:ClipCutter/code/state/
  echo "Stopping..."
  exit 0
}

docker pull ghcr.io/raimannma/clip-cutter:latest
docker network create nginx-proxy || true

rclone sync -P Nextcloud:ClipCutter/code/state/ state/
rclone sync -P Nextcloud:ClipCutter/code/users.json .
rclone sync -P Nextcloud:ClipCutter/code/model.onnx .
rclone sync -P Nextcloud:ClipCutter/code/docker-compose.yaml .

for channel in $(jq -r '.[] | .channel' users.json); do
  echo "Processing $channel"
  riot_ids=$(jq -r ".[] | select(.channel == \"$channel\") | .riot_ids | join(\",\")" users.json)
  echo "riot_ids: $riot_ids"
  for video in $(twitch-dl videos "$channel" --all -t archive --json | jq -r '.videos | .[] | select((now - (.publishedAt | fromdateiso8601)) < (3 * 24 * 3600)) | .id'); do
      clean
      echo "Processing $video";
      docker compose run --rm --entrypoint "clip-cutter -v $video -r '$riot_ids' --remove-matches" clip_cutter || continue;
      rclone move --ignore-existing -P clips/ Nextcloud:ClipCutter/"$channel"/ || continue;
  done
done

curl "https://status.manuel-hexe.de/api/push/VXB87TfmjS?status=up&msg=OK&ping=" || true

clean
stop
