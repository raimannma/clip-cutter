#!/usr/bin/bash

source /root/.bashrc

trap stop SIGINT

docker pull ghcr.io/raimannma/clip-cutter:latest

clean() {
  echo "Cleaning..."
  mkdir -p clips;
  rm -rf clips/*;

  mkdir -p matches;
  rm -rf matches/*;
}

stop() {
  echo "Stopping..."
  exit 0
}

for channel in $(jq -r '.[] | .channel' users.json); do
  echo "Processing $channel"
  riot_ids=$(jq -r ".[] | select(.channel == \"$channel\") | .riot_ids | join(\",\")" users.json)
  echo "riot_ids: $riot_ids"
  for video in $(twitch-dl videos "$channel" --all -t archive -j | jq -r '.videos | .[] | select((now - (.publishedAt | fromdateiso8601)) < (3 * 24 * 3600)) | .id'); do
      clean
      echo "Processing $video";
      docker compose run --rm --entrypoint "clip-cutter -v $video -r '$riot_ids' --remove-matches" clip_cutter || continue;
      rclone move --ignore-existing -P clips/ Nextcloud:ClipCutter/"$channel"/ || continue;
  done
done

curl "https://status.manuel-hexe.de/api/push/VXB87TfmjS?status=up&msg=OK&ping=" || true

clean
stop
