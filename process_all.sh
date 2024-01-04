#!/usr/bin/bash

source /root/.bashrc

docker-compose up -d redis --wait;

trap stop SIGINT

clean() {
  echo "Cleaning..."
  mkdir -p clips;
  rm -rf clips/*;

  mkdir -p matches;
  rm -rf matches/*;
}

stop() {
  echo "Stopping..."
  docker-compose down
  docker image rm ghcr.io/raimannma/clip-cutter
  exit 0
}

for channel in $(jq -r '.[] | .channel' users.json); do
  echo "Processing $channel"
  riot_ids=$(jq -r ".[] | select(.channel == \"$channel\") | .riot_ids | join(\",\")" users.json)
  echo "riot_ids: $riot_ids"
  for video in $(twitch-dl videos "$channel" --all -t archive -j | jq -r '.videos | .[] | select((now - (.publishedAt | fromdateiso8601)) < (2 * 24 * 3600)) | .id'); do
      clean
      echo "Processing $video";
      docker-compose run --rm --entrypoint "clip-cutter -v $video -r '$riot_ids' --remove-matches" clip_cutter || continue;
      rclone move -P clips/ Nextcloud:ClipCutter/"$channel"/ || continue;
  done
done

clean
stop
