# Summary

DLYT is a video downloader that fetches videos from provided URLs or YouTube playlists using `yt-dlp` and `ffmpeg`, and organizes them into directories based on the source file of the URLs.

DLYT is primarily used for downloading and organizing videos from the internet. It operates by reading URLs from `.urls` files located in a directory named `urls`. Each URL corresponds to a video or a YouTube playlist that DLYT will download using the `yt-dlp` and `ffmpeg` tools.

DLYT first checks if a directory named `urls` exists. If it doesn't, it creates the directory and a default file named `default.urls` inside it. The user can add URLs or YouTube playlist links to this file for downloading videos.

DLYT then reads all the `.urls` files in the `urls` directory. For each URL that doesn't start with '#', it downloads the video or all the videos in the playlist. The downloaded videos are stored in a directory named `videos`. If the `.urls` file is named `default.urls`, the videos are downloaded directly into the `videos` directory. Otherwise, they are downloaded into a subdirectory of `videos` with the same name as the `.urls` file.

In summary, DLYT is a handy tool for batch downloading and organizing videos from the internet. It requires the user to provide the URLs of the videos or YouTube playlists in `.urls` files, and it takes care of the rest. It's particularly useful for users who frequently download videos and want an automated and organized way to do so.

# Instructions for Use

1. Ensure that `yt-dlp` and `ffmpeg` are installed on your system. If not, follow the instructions provided by DLYT to install these dependencies.
2. Run DLYT. If the `urls` directory does not exist, DLYT will create it along with a `default.urls` file.
3. Add the URLs of the videos or YouTube playlists you want to download to the `default.urls` file or to a new `.urls` file in the `urls` directory. Each URL should be on a new line. Lines starting with '#' are considered comments and are ignored.
4. Run DLYT again. It will download the videos from the URLs provided in the `.urls` files and store them in the `videos` directory or its subdirectories.
5. Enjoy your downloaded videos!

## CLI Flags

- `--update-ytdlp` - update `yt-dlp` before starting downloads.
- `--no-aria2c` - disable the use of `aria2c` as an external downloader.
- `--prefer-aria2c` - warn if `aria2c` is not installed and prefer using it when available.
- `--use-aria2c` - force using `aria2c` even for YouTube links.
- `--force-best-quality` - use the highest quality even if it's a throttled VP9/AV1 format. Without this flag, DLYT auto-downgrades to fast MP4 1080p when throttling is detected.
- `--probe` - perform an initial format probe to warn about throttled formats (slightly slower startup).

Please remember to replace the placeholders in the URLs with actual values before running DLYT. Happy downloading!


## Building

This project uses [Cargo](https://doc.rust-lang.org/cargo/) as its build system. To compile DLYT from source, run:

```bash
cargo build --release
```

The compiled binary will be located in `target/release/dlyt`.

Alternatively, if you want to install it, run:

```bash
cargo install --path .
```

The compiled binary will be located in `~/.cargo/bin` which should already be in your PATH.
