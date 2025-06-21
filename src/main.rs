use clap::Parser;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Force yt-dlp to update before running
    #[arg(long)]
    update_ytdlp: bool,

    /// Disable aria2c even if installed
    #[arg(long)]
    no_aria2c: bool,

    /// Prefer aria2c and warn if it's unavailable
    #[arg(long)]
    prefer_aria2c: bool,

    /// Force use of aria2c even for YouTube
    #[arg(long)]
    use_aria2c: bool,

    /// Use best available quality even if it's throttled (e.g., itag=313 VP9)
    #[arg(long)]
    force_best_quality: bool,

    /// Skip probing formats with yt-dlp -J for faster startup
    #[arg(long)]
    skip_probe: bool,
}

fn command_exists(cmd: &str) -> bool {
    let output = if cfg!(target_os = "windows") {
        Command::new("where").arg("/Q").arg(cmd).output()
    } else {
        Command::new("which").arg(cmd).output()
    };

    output.map_or(false, |o| o.status.success())
}

fn is_ytdlp_outdated() -> Result<bool, std::io::Error> {
    let output = Command::new("yt-dlp")
        .args(["-U", "--", "--no-update"])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let is_outdated = stdout.contains("Latest version:")
        && stdout.contains("Current version:")
        && !stdout.contains("yt-dlp is up to date");

    Ok(is_outdated)
}

fn aria2c_available() -> bool {
    which::which("aria2c").is_ok()
}

fn get_domain(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
}

fn extract_formats(url: &str) -> io::Result<(bool, bool)> {
    let output = Command::new("yt-dlp").args(["-J", url]).output()?;
    if !output.status.success() {
        return Ok((false, false));
    }

    let json: Value = serde_json::from_slice(&output.stdout)?;
    let formats = json
        .get("formats")
        .and_then(|f| f.as_array())
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "invalid formats"))?;

    let mut has_mp4_1080 = false;
    let mut best_height = 0u64;
    let mut best_id = String::new();
    let mut best_ext = String::new();
    let mut best_vcodec = String::new();

    for f in formats {
        let id = f.get("format_id").and_then(|v| v.as_str()).unwrap_or("");
        let ext = f.get("ext").and_then(|v| v.as_str()).unwrap_or("");
        let height = f.get("height").and_then(|v| v.as_u64()).unwrap_or(0);
        let vcodec = f.get("vcodec").and_then(|v| v.as_str()).unwrap_or("");

        if ext == "mp4" && height <= 1080 && height > 0 {
            has_mp4_1080 = true;
        }

        if vcodec != "none" && height > best_height {
            best_height = height;
            best_id = id.to_string();
            best_ext = ext.to_string();
            best_vcodec = vcodec.to_string();
        }
    }

    let best_is_throttled = matches!(best_id.as_str(), "313" | "248" | "271" | "308" | "315")
        || (best_ext == "webm" && (best_vcodec.starts_with("vp9") || best_vcodec.starts_with("av01")));

    Ok((has_mp4_1080, best_is_throttled))
}

fn select_format(url: &str, force_best_quality: bool) -> io::Result<(String, bool, bool)> {
    let (has_mp4_1080, best_is_throttled) = extract_formats(url)?;

    if force_best_quality {
        return Ok(("bestvideo+bestaudio".to_string(), best_is_throttled, false));
    }

    if best_is_throttled && has_mp4_1080 {
        Ok((
            "bestvideo[ext=mp4][height<=1080]+bestaudio[ext=m4a]/best[ext=mp4]".to_string(),
            false,
            true,
        ))
    } else {
        Ok(("bestvideo+bestaudio/best".to_string(), false, false))
    }
}

fn select_format_without_probe(force_best_quality: bool) -> (String, bool, bool) {
    if force_best_quality {
        ("bestvideo+bestaudio/best".to_string(), false, false)
    } else {
        (
            "bestvideo[ext=mp4][height<=1080]+bestaudio[ext=m4a]/best[ext=mp4]"
                .to_string(),
            false,
            false,
        )
    }
}

fn check_dependencies() -> bool {
    if !command_exists("yt-dlp") || !command_exists("ffmpeg") {
        println!("The required dependencies yt-dlp and ffmpeg are not installed.");
        println!("Please install them before running this program.");
        println!("On Linux, you can use the following commands:");
        println!("sudo curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp");
        println!("sudo chmod a+rx /usr/local/bin/yt-dlp");
        println!("sudo apt-get install ffmpeg");
        println!("On Windows, you can download the executables and add them to your PATH:");
        println!("yt-dlp: https://github.com/yt-dlp/yt-dlp/releases/latest");
        println!("ffmpeg: https://www.gyan.dev/ffmpeg/builds/");
        false
    } else {
        true
    }
}

fn create_default_structure(dir_path: &str) -> io::Result<bool> {
    if !Path::new(dir_path).exists() {
        fs::create_dir(dir_path)?;
        println!(
            "Created directory: {}. You can create your own .urls files in this directory. The name of the file will be used as the subdirectory for the downloaded videos.",
            dir_path
        );

        let default_file = Path::new(dir_path).join("default.urls");
        let mut file = File::create(&default_file)?;
        writeln!(
            file,
            "# Add your URLs here, one per line. This is the default file, videos will be downloaded to the base directory."
        )?;
        println!(
            "Created file: {}. You can add URLs to this file for downloading videos. For different subdirectories, create a new .urls file with the name of the subdirectory.",
            default_file.display()
        );
        return Ok(true);
    }

    let default_file = Path::new(dir_path).join("default.urls");
    if !default_file.exists() {
        let mut file = File::create(&default_file)?;
        writeln!(
            file,
            "# Add your URLs here, one per line. This is the default file, videos will be downloaded to the base directory."
        )?;
        println!(
            "Created file: {}. You can add URLs to this file for downloading videos. For different subdirectories, create a new .urls file with the name of the subdirectory.",
            default_file.display()
        );
        return Ok(true);
    }

    Ok(false)
}

fn process_url_files(
    dir_path: &str,
    base_dir: &str,
    archive_file: &str,
    base_use_aria2c: bool,
    force_aria2c: bool,
    force_best_quality: bool,
    skip_probe: bool,
) -> io::Result<bool> {
    let mut urls_exist = false;

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_stem = path.file_stem().unwrap().to_str().unwrap();
            let output_dir = if file_stem == "default" {
                PathBuf::from(base_dir)
            } else {
                PathBuf::from(base_dir).join(file_stem)
            };
            fs::create_dir_all(&output_dir)?;

            let file = File::open(&path)?;
            let reader = io::BufReader::new(file);

            for line in reader.lines() {
                let url = line?;
                if url.trim().is_empty() || url.starts_with('#') {
                    continue;
                }

                urls_exist = true;
                let domain = get_domain(&url).unwrap_or_default();
                let is_youtube = domain.contains("youtube.com") || domain.contains("youtu.be");

                let (format_str, warn_throttled, downgraded) = if is_youtube {
                    if skip_probe {
                        select_format_without_probe(force_best_quality)
                    } else {
                        select_format(&url, force_best_quality)?
                    }
                } else if force_best_quality {
                    ("bestvideo+bestaudio/best".to_string(), false, false)
                } else {
                    ("best".to_string(), false, false)
                };

                if force_aria2c && is_youtube {
                    eprintln!("Warning: Using aria2c on YouTube may result in slow downloads.");
                }

                if warn_throttled {
                    eprintln!(
                        "\u{26A0}\u{FE0F} WARNING: You are downloading a VP9 or AV1 format (e.g. itag=313), which is often throttled by YouTube.\nDownload speed may be extremely slow. Use --no-force-best-quality to allow auto-downgrade to faster formats."
                    );
                }

                if downgraded {
                    println!(
                        "Auto-selected mp4 1080p+ audio to avoid YouTube throttling. Use --force-best-quality to override."
                    );
                }

                let use_aria = if force_aria2c {
                    true
                } else if base_use_aria2c {
                    !is_youtube
                } else {
                    false
                };

                let mut cmd = Command::new("yt-dlp");
                cmd.arg(&url)
                    .arg("--download-archive")
                    .arg(archive_file)
                    .arg("--user-agent")
                    .arg("Mozilla/5.0")
                    .arg("-f")
                    .arg(&format_str)
                    .arg("--prefer-ffmpeg")
                    .arg("--write-description")
                    .arg("--add-metadata")
                    .arg("--write-auto-sub")
                    .arg("--embed-subs");

                if use_aria {
                    cmd.args([
                        "--external-downloader",
                        "aria2c",
                        "--external-downloader-args",
                        "-x 4 -k 1M",
                    ]);
                } else {
                    cmd.args(["--concurrent-fragments", "10", "--no-part"]);
                }

                cmd.arg("-o")
                    .arg(output_dir.join("%(title)s.%(ext)s").to_str().unwrap());

                let status = cmd.status()?;
                println!("Download finished with exit status: {}", status);
            }
        }
    }

    Ok(urls_exist)
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    if !check_dependencies() {
        return Ok(());
    }

    if args.update_ytdlp {
        let status = Command::new("yt-dlp").arg("-U").status()?;
        if !status.success() {
            eprintln!("Warning: Failed to update yt-dlp.");
        }
    } else if is_ytdlp_outdated()? {
        eprintln!(
            "Warning: yt-dlp is outdated. Run `yt-dlp -U` to update and avoid throttling or bugs."
        );
    }

    let prefer_aria2c = args.prefer_aria2c;
    let base_use_aria2c = !args.no_aria2c && aria2c_available();
    let force_aria2c = args.use_aria2c;
    let force_best_quality = args.force_best_quality;

    if prefer_aria2c && !aria2c_available() && !args.no_aria2c {
        eprintln!("aria2c not found. Install it with `sudo apt install aria2` or disable with --no-aria2c.");
    }

    let dir_path = "urls";
    let base_dir = "videos";
    let archive_file = "downloaded.txt";

    let created = create_default_structure(dir_path)?;
    if created {
        return Ok(());
    } else {
        println!("Found existing {} directory. Processing .urls files...", dir_path);
    }

    let urls_exist = process_url_files(
        dir_path,
        base_dir,
        archive_file,
        base_use_aria2c,
        force_aria2c,
        force_best_quality,
        args.skip_probe,
    )?;

    if !urls_exist {
        println!("No URLs found in the .urls files. Please add URLs to the .urls files for downloading videos. Each URL should be on a new line. Lines starting with '#' are considered comments and are ignored.");
    }

    Ok(())
}
