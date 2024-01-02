#!/bin/bash

mkdir -p clips;
rm -rf clips/*;

mkdir -p matches;
rm -rf matches/*;

for channel in $(jq -r '.[] | .channel' users.json); do
  echo "Processing $channel"
  riot_ids=$(jq -r ".[] | select(.channel == \"$channel\") | .riot_ids | join(\",\")" users.json)
  echo "riot_ids: $riot_ids"
  for video in $(twitch-dl videos "$channel" --all -t archive -j | jq '.videos' | jq -r '.[].id'); do
      echo "Processing $video";
      docker-compose run --rm --entrypoint "clip-cutter -v $video -r '$riot_ids' --remove-matches" clip_cutter;
      exit 0;
      rclone move -P clips/ Nextcloud:ClipCutter/New/"$channel"/;
      rm -r clips/*;
  done
done
