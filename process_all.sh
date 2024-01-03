#!/bin/bash

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

clean

for channel in $(jq -r '.[] | .channel' users.json); do
  echo "Processing $channel"
  riot_ids=$(jq -r ".[] | select(.channel == \"$channel\") | .riot_ids | join(\",\")" users.json)
  echo "riot_ids: $riot_ids"
  for video in $(twitch-dl videos "$channel" --all -t archive -j | jq '.videos' | jq -r '.[].id'); do
      echo "Processing $video";
      value_exists=$(redis-cli SISMEMBER processed "$video")
      if [ "$value_exists" -eq 1 ]; then
        echo "Video $video already processed. Skipping..."
        continue
      fi
      docker-compose run --rm --entrypoint "clip-cutter -v $video -r '$riot_ids' --remove-matches" clip_cutter || break;
      rclone move -P clips/ Nextcloud:ClipCutter/"$channel"/ || break;
      clean
      redis-cli SADD processed "$video"
  done
done

clean
stop
