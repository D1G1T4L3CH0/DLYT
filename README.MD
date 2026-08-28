# Summary

DLYT is a video downloader that fetches videos from provided URLs or YouTube playlists using `yt-dlp` and `ffmpeg`, and organizes them into directories based on the source file of the URLs.

DLYT is primarily used for downloading and organizing videos from the internet. It operates by reading URLs from `.urls` files located in a directory named `urls`. Each URL corresponds to a video or a YouTube playlist that DLYT will download using the `yt-dlp` and `ffmpeg` tools.

DLYT first checks if a directory named `urls` exists. If it doesn't, it creates the directory and a default file named `default.urls` inside it. The user can add URLs or YouTube playlist links to this file for downloading videos.

DLYT then reads all the `.urls` files in the `urls` directory. For each URL that doesn't start with '#', it downloads the video or all the videos in the playlist. The downloaded videos are stored in a directory named `videos`. If the `.urls` file is named `default.urls`, the videos are downloaded directly into the `videos` directory. Otherwise, they are downloaded into a subdirectory of `videos` with the same name as the `.urls` file.

In summary, DLYT is a handy tool for batch downloading and organizing videos from the internet. It requires the user to provide the URLs of the videos or YouTube playlists in `.urls` files, and it takes care of the rest. It's particularly useful for users who frequently download videos and want an automated and organized way to do so.

## Build

```sh
cargo build --release
```

The binary is at `target/release/dlyt`. Run it from the directory that should contain `urls/` and `videos/`.

To install it on your `PATH` instead:

```sh
cargo install --path .
```

This places the binary in `~/.cargo/bin`.

# Instructions for Use

1. Ensure that `yt-dlp` and `ffmpeg` are installed on your system. `yt-dlp` can also be installed with `pipx install yt-dlp` or a distro package such as `sudo pacman -S yt-dlp` or `sudo apt install yt-dlp`. If not, follow the instructions provided by DLYT to install these dependencies.
2. Run DLYT. If the `urls` directory does not exist, DLYT will create it along with a `default.urls` file.
3. Add the URLs of the videos or YouTube playlists you want to download to the `default.urls` file or to a new `.urls` file in the `urls` directory. Each URL should be on a new line. Lines starting with '#' are considered comments and are ignored.
4. Run DLYT again. It will download the videos from the URLs provided in the `.urls` files and store them in the `videos` directory or its subdirectories.
5. Enjoy your downloaded videos!

## CLI Flags

- `--update-ytdlp` - run `yt-dlp -U` before downloading.
- `--no-aria2c` - never use aria2c.
- `--prefer-aria2c` - warn if aria2c is missing; use it when available (non-YouTube).
- `--use-aria2c` - force aria2c for all links, including YouTube.
- `--throttle` - pace requests to dodge rate limits / the bot wall: ~1s between API requests, a random 2-6s pause before each download, and 2 parallel fragments instead of 4.
- `--cookies-from-browser <BROWSER[:PROFILE]>` - hand browser cookies to yt-dlp (e.g. `firefox`, `chrome:Default`).
- `--cookies <FILE>` - hand a Netscape-format cookies file to yt-dlp.
- The default number of parallel download fragments is 4 (lowered from 10).
- `--quality <balanced|best|compat|small|audio>` - quality preset (default `balanced`):
  - `balanced` - ~1080p, H.264, MP4/M4A. Fast, plays everywhere. *(default)*
  - `best` - highest resolution + fps, any codec (VP9/AV1/4K).
  - `compat` - highest resolution, but forced H.264 + MP4/M4A.
  - `small` - capped at 720p, MP4.
  - `audio` - audio only, original codec, no re-encode (subtitles skipped; pair with
    `--nfo-type musicvideo` or `--no-nfo`).
- `--force-best-quality` - deprecated alias for `--quality best`.
- `--nfo-type <movie|musicvideo>` - Kodi/Jellyfin NFO flavor to write (default `movie`).
- `--no-nfo` - do not write `.nfo` sidecars.
- `--force-nfo` - overwrite existing `.nfo` sidecars instead of skipping them.
- `--refresh-metadata` - for every URL in your `.urls` files, fetch metadata only (no video download, ignores `downloaded.txt`) and (re)build every `.nfo` sidecar and the manifest. Run once to backfill an existing library.
- `--redownload <URL>...` - re-fetch specific videos, overwriting existing files and ignoring the archive. The refetch lands back in the folder the video already lives in (falls back to `videos/`). Handy for re-grabbing one video from a bulk-added playlist.

## Metadata & Jellyfin

Each download also writes, next to the video: `<name>.nfo` (Kodi/Jellyfin sidecar), `<name>.info.json` (yt-dlp raw metadata), and `<name>.jpg` (thumbnail).

The video description is stored in the `.nfo` (`<plot>`), the embedded metadata, and `<name>.info.json`.

`videos/downloads.tsv` is a human-readable manifest of every downloaded video (`uploaded`, `id`, `title`, `file`), rewritten on each run. `downloaded.txt` remains yt-dlp's internal archive of bare video IDs.

A normal re-run will not rebuild sidecars for videos already in `downloaded.txt` (yt-dlp skips them entirely) - use `--refresh-metadata`.

The thumbnail is embedded in the file and also left on disk as a same-named `.jpg`, so Jellyfin uses it as the primary image.

In Jellyfin, add a library over the `videos/` tree; keep the default "Nfo" metadata reader enabled; optionally disable internet metadata providers so the sidecar wins.

The `.nfo` flavor is `<movie>` by default for Movies, mixed, or Home-videos libraries, or `<musicvideo>` for a Music Videos library.

## Forwarding options to yt-dlp

Anything after `--` is passed straight through to `yt-dlp`, appended last so it can
override DLYT's own `-f` / `-S` / `-o`:

```sh
dlyt --quality best -- --sponsorblock-remove all --cookies-from-browser firefox
```

DLYT does not pass `--ignore-config`, so your `~/.config/yt-dlp/config` (or a local
`yt-dlp.conf`) still applies as well.

## Rate limits & the bot wall

DLYT runs anonymously by default - YouTube can only throttle your IP, there is no
account to ban.

If you hit `Sign in to confirm you're not a bot`:

1. Run `yt-dlp -U` first. The wall is often transient or fixed by an extractor update.
2. Add `--throttle`.
3. Last resort: cookies from a **dedicated throwaway Google account** - heavy automated
   use can get an account locked, so never your main one. Pass them with
   `--cookies-from-browser firefox` (you must stay logged in there; cookies expire) or
   `--cookies cookies.txt`. A cookies file is a live session - keep it private
   (`chmod 600`, and it's gitignored).

VPN and datacenter IPs are often pre-flagged; VPN + cookies usually means *more*
challenges, not fewer.

Please remember to replace the placeholders in the URLs with actual values before running DLYT. Happy downloading!
