import os
import subprocess
from dataclasses import dataclass
from datetime import datetime
from typing import List, Optional
import sys

subprocess.check_call([sys.executable, "-m", "pip", "install", "flask"])

from flask import Flask, Response

app = Flask(__name__)
base_dir = os.environ.get("BASE_DIR", "clips-public")

THUMBS_DIR = "/tmp/thumbs"


@dataclass
class Clip:
    streamer: str
    gamemode: str
    category: str
    file_name: str
    timestamp: float
    map_name: Optional[str] = None
    agent_name: Optional[str] = None
    args: Optional[List[str]] = None

    @classmethod
    def from_path(cls, path: str):
        *_, streamer, gamemode, category, filename = os.path.splitext(path)[0].split(os.sep)
        date, time, *args = filename.split("_")
        date_time = datetime.strptime(date + " " + time, "%d-%m-%Y %H-%M-%S")
        return Clip(streamer, gamemode, category, filename, date_time.timestamp(), args.pop(0), args.pop(0), args)


@app.get("/favicon.ico")
def icon():
    return Response(open('favicon.ico', "rb").read(), mimetype="image/x-icon")


@app.get("/")
def index():
    return Response(open('index.html').read(), mimetype="text/html")


@app.get("/names")
def get_names():
    return os.listdir(base_dir)


@app.get("/gamemodes/<name>")
def get_gamemodes(name: str):
    return os.listdir(os.path.join(base_dir, name))


@app.get("/categories/<name>/<gamemode>")
def get_category(name: str, gamemode: str):
    return os.listdir(os.path.join(base_dir, name, gamemode))


@app.get("/clips/<name>/<gamemode>/<category>")
def get_clips(name: str, gamemode: str, category: str):
    path = os.path.join(base_dir, name, gamemode, category)
    return [Clip.from_path(os.path.join(path, p)) for p in os.listdir(path) if p.endswith(".mp4")]


@app.get("/clips/<name>/<gamemode>/<category>/<clip>/thumbnail")
def get_thumbnail(name: str, gamemode: str, category: str, clip: str):
    video_path = os.path.join(base_dir, name, gamemode, category, clip + ".mp4")
    thumb_path = os.path.join(THUMBS_DIR, name, gamemode, category, clip + ".png")
    os.makedirs(os.path.dirname(thumb_path), exist_ok=True)
    if not os.path.exists(thumb_path):
        subprocess.call(['ffmpeg', '-i', video_path, '-ss', '00:00:00.000', '-vframes', '1', '-y', thumb_path])

    return Response(open(thumb_path, "rb").read(), mimetype="image/png")


@app.get("/clips/<name>/<gamemode>/<category>/<clip>/video")
def get_video(name: str, gamemode: str, category: str, clip: str):
    video_path = os.path.join(base_dir, name, gamemode, category, clip + ".mp4")
    return Response(open(video_path, "rb").read(), mimetype="video/mp4")


if __name__ == '__main__':
    app.run(host="0.0.0.0")
