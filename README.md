# JellyMal-rs

JellyMal-rs is a tool that allows you to automatically update your MyAnimeList status as you watch shows in Jellyfin. It runs as a Docker container, providing a seamless integration between your Jellyfin media server and MyAnimeList.

## Features

- Automatic synchronization of your Jellyfin watch history with MyAnimeList
- Dockerized for easy deployment and management

## How does it work?

`jellymal-rs` runs as a Docker container that regularly polls your Jellyfin API for watch status. It then uses OAuth to hit the MyAnimeList API and syncs the watch status to your MyAnimeList profile.

> **Note:** https://raw.githubusercontent.com/Anime-Lists/anime-lists/master/anime-list-master.xml is used to map TVDB IDs (Jellyfin) to AniDB IDs. And https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json is used to map AniDB IDs to MyAnimeList IDs. This allows `jellymal-rs` to accurately map the shows you are watching to the correct MyAnimeList show.

## Prerequisites

Before getting started, make sure you have the following installed:

- Docker: [Installation Guide](https://docs.docker.com/get-docker/)
- Jellyfin media server: [Official Website](https://jellyfin.org/)

You'll also need to make sure you've registered a new MAL client with the MAL API. To do this, follow the official MAL instructions [here](https://myanimelist.net/apiconfig).

## Installation
`jellymal-rs` runs as a docker container, that regularly polls your J

   ```bash
   git clone https://github.com/pgeske/jellymal-rs
   sudo docker build -t jellymal:latest jellymal-rs/
   ```

## Setup
### First Time
For the first time setup, we'll run the container directly. That's because we'll need to interact with it to initialize the MAL oauth token for the first time. After they've been acquired and saved (into the `/data` directory), `jellymal-rs` will automatically refresh them before they expire. So this manual interaction is only required once, for first time setup.
```bash
# create the data directory that you are mounting in your compose, and make sure
# the user the container runs as can write to it
mkdir /path/to/your/data
sudo chown -R 1000:1000 /path/to/your/data

# start the container so that we can interact with it to acquire the initial tokens
sudo docker compose -f /path/to/your/docker-compose.yml run --rm jellymal
```

After that, follow the instructions presented to you, and quit out of the container (`Ctrl-C`) when complete. 

### After First Time
Just kick off the container as part of your normal docker compose (or other) setup.
```
sudo docker compose up /path/to/your/docker-compose.yml
```