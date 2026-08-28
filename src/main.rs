use clap::Parser;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use url::Url;

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
enum NfoType {
    Movie,
    Musicvideo,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
enum Quality {
    #[default]
    Balanced,
    Best,
    Compat,
    Small,
    Audio,
}

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

    /// Pace requests to avoid rate limits / the "confirm you're not a bot" wall
    #[arg(long)]
    throttle: bool,

    /// Load cookies from a browser (yt-dlp syntax, e.g. "firefox", "chrome:Default")
    #[arg(long, value_name = "BROWSER[:PROFILE]")]
    cookies_from_browser: Option<String>,

    /// Load cookies from a Netscape-format file
    #[arg(long, value_name = "FILE", conflicts_with = "cookies_from_browser")]
    cookies: Option<PathBuf>,

    /// Video/audio quality preset
    #[arg(long, value_enum, default_value_t = Quality::Balanced)]
    quality: Quality,

    /// Deprecated alias for `--quality best`
    #[arg(long, hide = true)]
    force_best_quality: bool,

    /// Extra arguments forwarded verbatim to yt-dlp (everything after `--`)
    #[arg(last = true, allow_hyphen_values = true, value_name = "YT_DLP_ARG")]
    ytdlp_extra: Vec<String>,

    /// Sidecar NFO flavor for Jellyfin/Kodi
    #[arg(long, value_enum, default_value_t = NfoType::Movie)]
    nfo_type: NfoType,

    /// Do not write .nfo sidecars
    #[arg(long)]
    no_nfo: bool,

    /// Overwrite existing .nfo sidecars instead of skipping them
    #[arg(long)]
    force_nfo: bool,

    /// Re-fetch metadata for every URL in the lists (no video download) and rebuild all .nfo sidecars
    #[arg(long)]
    refresh_metadata: bool,

    /// Re-download specific videos: bypass the archive, overwrite existing files, rebuild sidecars
    #[arg(long, num_args = 1.., value_name = "URL")]
    redownload: Vec<String>,
}

#[derive(Debug)]
enum Aria2cMode {
    Off,
    Auto,
    Force,
}

struct RunConfig {
    base_dir: PathBuf,
    archive_file: PathBuf,
    aria2c: Aria2cMode,
    quality: Quality,
    ytdlp_extra: Vec<String>,
    nfo_type: NfoType,
    write_nfo: bool,
    force_nfo: bool,
    refresh_metadata: bool,
    throttle: bool,
    cookies_from_browser: Option<String>,
    cookies: Option<PathBuf>,
}

fn format_args(is_youtube: bool, quality: Quality) -> Vec<String> {
    let yt_sel = if is_youtube { "bv*+ba/b" } else { "b/bv*+ba" };
    let parts: Vec<&str> = match quality {
        Quality::Balanced => vec![
            "-f",
            yt_sel,
            "-S",
            "res:1080,fps,vcodec:h264,ext:mp4:m4a,acodec:m4a",
        ],
        Quality::Best => vec!["-f", yt_sel, "-S", "res,fps"],
        Quality::Compat => vec![
            "-f",
            yt_sel,
            "-S",
            "res,fps,vcodec:h264,ext:mp4:m4a,acodec:m4a",
        ],
        Quality::Small => vec![
            "-f",
            "bv*[height<=720]+ba/b[height<=720]/bv*+ba/b",
            "-S",
            "res:720,fps,ext:mp4:m4a",
        ],
        Quality::Audio => vec![
            "-f",
            "ba/b",
            "-x",
            "--audio-format",
            "best",
            "--audio-quality",
            "0",
            "-S",
            "abr",
        ],
    };
    parts.into_iter().map(str::to_string).collect()
}

fn get_domain(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
}

fn check_dependencies() -> bool {
    if !which::which("yt-dlp").is_ok() || !which::which("ffmpeg").is_ok() {
        println!("The required dependencies yt-dlp and ffmpeg are not installed.");
        println!("Please install them before running this program.");
        println!("On Linux, you can use the following commands:");
        println!(
            "sudo curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp"
        );
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

fn add_common_flags(cmd: &mut Command, cfg: &RunConfig, is_youtube: bool) {
    cmd.arg("--write-info-json")
        .arg("--no-write-playlist-metafiles")
        .arg("--write-thumbnail")
        .arg("--convert-thumbnails")
        .arg("jpg")
        .arg("--embed-thumbnail")
        .arg("--embed-metadata")
        .arg("--embed-chapters");
    if !matches!(cfg.quality, Quality::Audio) {
        cmd.arg("--write-auto-subs")
            .arg("--sub-langs")
            .arg("en.*")
            .arg("--embed-subs");
    }
    cmd.args(format_args(is_youtube, cfg.quality));
}

fn add_net_flags(cmd: &mut Command, cfg: &RunConfig) {
    if let Some(spec) = &cfg.cookies_from_browser {
        cmd.arg("--cookies-from-browser").arg(spec);
    } else if let Some(path) = &cfg.cookies {
        cmd.arg("--cookies").arg(path);
    }
    if cfg.throttle {
        cmd.args([
            "--sleep-requests",
            "1",
            "--sleep-interval",
            "2",
            "--max-sleep-interval",
            "6",
        ]);
    }
}

fn download_one(url: &str, output_dir: &Path, cfg: &RunConfig) -> io::Result<()> {
    let domain = get_domain(url).unwrap_or_default();
    let is_youtube = domain.contains("youtube.com") || domain.contains("youtu.be");
    let use_aria = match cfg.aria2c {
        Aria2cMode::Force => true,
        Aria2cMode::Auto => !is_youtube,
        Aria2cMode::Off => false,
    };

    if use_aria && is_youtube {
        eprintln!("Warning: Using aria2c on YouTube may result in slow downloads.");
    }

    let mut cmd = Command::new("yt-dlp");
    cmd.arg(url)
        .arg("--download-archive")
        .arg(&cfg.archive_file);
    add_net_flags(&mut cmd, cfg);
    add_common_flags(&mut cmd, cfg, is_youtube);

    if use_aria {
        let dl_args = if cfg.throttle {
            "aria2c:-x 2 -k 1M"
        } else {
            "aria2c:-x 4 -k 1M"
        };
        cmd.args([
            "--external-downloader",
            "aria2c",
            "--external-downloader-args",
            dl_args,
        ]);
    } else {
        let frags = if cfg.throttle { "2" } else { "4" };
        cmd.args(["--concurrent-fragments", frags]);
    }

    cmd.arg("-o")
        .arg(output_dir.join("%(title)s [%(id)s].%(ext)s"));

    if !cfg.ytdlp_extra.is_empty() {
        cmd.args(&cfg.ytdlp_extra);
    }

    let status = cmd.status()?;
    println!("Download finished with exit status: {}", status);
    Ok(())
}

fn refresh_one(url: &str, output_dir: &Path, cfg: &RunConfig) -> io::Result<()> {
    let mut cmd = Command::new("yt-dlp");
    cmd.arg(url)
        .arg("--skip-download")
        .arg("--no-download-archive");
    add_net_flags(&mut cmd, cfg);
    cmd.arg("--write-info-json")
        .arg("--no-write-playlist-metafiles")
        .arg("--write-thumbnail")
        .arg("--convert-thumbnails")
        .arg("jpg")
        .arg("-o")
        .arg(output_dir.join("%(title)s [%(id)s].%(ext)s"));
    if !cfg.ytdlp_extra.is_empty() {
        cmd.args(&cfg.ytdlp_extra);
    }
    let status = cmd.status()?;
    println!("Metadata refresh finished with exit status: {status}");
    Ok(())
}

fn process_url_files(dir_path: &Path, cfg: &RunConfig) -> io::Result<bool> {
    let mut urls_exist = false;

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("urls") {
            let stem = match path.file_stem() {
                Some(stem) => stem,
                None => continue,
            };
            let output_dir = if stem == std::ffi::OsStr::new("default") {
                cfg.base_dir.clone()
            } else {
                cfg.base_dir.join(stem)
            };
            fs::create_dir_all(&output_dir)?;

            let file = File::open(&path)?;
            let reader = io::BufReader::new(file);

            for line in reader.lines() {
                let raw_line = match line {
                    Ok(line) => line,
                    Err(e) => {
                        eprintln!("error reading URL line in {}: {e}", path.display());
                        continue;
                    }
                };
                let url = raw_line.trim();
                if url.is_empty() || url.starts_with('#') {
                    continue;
                }

                urls_exist = true;
                let res = if cfg.refresh_metadata {
                    refresh_one(url, &output_dir, cfg)
                } else {
                    download_one(url, &output_dir, cfg)
                };
                if let Err(e) = res {
                    eprintln!("error processing {url}: {e}");
                    continue;
                }
            }

            if cfg.write_nfo
                && let Err(e) = generate_nfos(&output_dir, cfg, cfg.refresh_metadata)
            {
                eprintln!(
                    "error generating .nfo sidecars in {}: {e}",
                    output_dir.display()
                );
            }
            if let Err(e) = update_index(&output_dir, cfg) {
                eprintln!("index update failed for {}: {e}", output_dir.display());
            }
        }
    }

    Ok(urls_exist)
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

fn nfo_dates(info: &serde_json::Value) -> Option<(String, String)> {
    let raw = info
        .get("release_date")
        .and_then(|v| v.as_str())
        .or_else(|| info.get("upload_date").and_then(|v| v.as_str()))?;
    if raw.len() != 8 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((
        raw[0..4].to_string(),
        format!("{}-{}-{}", &raw[0..4], &raw[4..6], &raw[6..8]),
    ))
}

fn nfo_runtime_min(info: &serde_json::Value) -> Option<i64> {
    let secs = info.get("duration").and_then(|v| v.as_f64())?;
    let mins = (secs / 60.0).round() as i64;
    if mins > 0 { Some(mins) } else { None }
}

fn first_nonempty<'a>(info: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    for k in keys {
        if let Some(s) = info.get(*k).and_then(|v| v.as_str())
            && !s.trim().is_empty()
        {
            return Some(s);
        }
    }
    None
}

fn build_nfo(info: &serde_json::Value, nfo_type: NfoType) -> String {
    let mut b = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let id = info.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let title = info.get("title").and_then(|v| v.as_str()).unwrap_or(id);
    let description = info
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let creator = first_nonempty(info, &["uploader", "channel", "uploader_id"]);
    let dates = nfo_dates(info);
    let runtime = nfo_runtime_min(info);

    let push_el = |buf: &mut String, tag: &str, val: &str| {
        if !val.trim().is_empty() {
            let _ = writeln!(buf, "  <{tag}>{}</{tag}>", xml_escape(val));
        }
    };

    let root = match nfo_type {
        NfoType::Movie => "movie",
        NfoType::Musicvideo => "musicvideo",
    };
    let _ = writeln!(b, "<{root}>");

    match nfo_type {
        NfoType::Movie => {
            push_el(&mut b, "title", title);
            push_el(&mut b, "originaltitle", title);
            push_el(&mut b, "plot", description);
            push_el(&mut b, "outline", description);
            if let Some((year, premiered)) = &dates {
                push_el(&mut b, "year", year);
                push_el(&mut b, "premiered", premiered);
                push_el(&mut b, "releasedate", premiered);
            }
            if let Some(c) = creator {
                push_el(&mut b, "studio", c);
                push_el(&mut b, "director", c);
            }
        }
        NfoType::Musicvideo => {
            let track = first_nonempty(info, &["track"]).unwrap_or(title);
            let artist = first_nonempty(info, &["artist"]).or(creator).unwrap_or("");
            push_el(&mut b, "title", track);
            push_el(&mut b, "artist", artist);
            push_el(&mut b, "plot", description);
            if let Some((year, premiered)) = &dates {
                push_el(&mut b, "year", year);
                push_el(&mut b, "premiered", premiered);
            }
            if let Some(c) = creator {
                push_el(&mut b, "studio", c);
            }
        }
    }

    if let Some(mins) = runtime {
        let _ = writeln!(b, "  <runtime>{mins}</runtime>");
    }
    if let Some(cats) = info.get("categories").and_then(|v| v.as_array()) {
        for c in cats.iter().filter_map(|v| v.as_str()) {
            push_el(&mut b, "genre", c);
        }
    }
    if let Some(tags) = info.get("tags").and_then(|v| v.as_array()) {
        for t in tags.iter().filter_map(|v| v.as_str()).take(25) {
            push_el(&mut b, "tag", t);
        }
    }
    if !id.is_empty() {
        let _ = writeln!(
            b,
            "  <uniqueid type=\"youtube\" default=\"true\">{}</uniqueid>",
            xml_escape(id)
        );
        push_el(&mut b, "id", id);
    }
    b.push_str("  <lockdata>false</lockdata>\n");
    let _ = writeln!(b, "</{root}>");
    b
}

fn generate_nfos(dir: &Path, cfg: &RunConfig, force: bool) -> io::Result<()> {
    let mut wrote = 0;
    for entry in fs::read_dir(dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("error reading dir entry in {}: {e}", dir.display());
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let base = match name.strip_suffix(".info.json") {
            Some(b) => b,
            None => continue,
        };
        let nfo_path = dir.join(format!("{base}.nfo"));
        if nfo_path.exists() && !(force || cfg.force_nfo) {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error reading {}: {e}", path.display());
                continue;
            }
        };
        let info: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error parsing {}: {e}", path.display());
                continue;
            }
        };
        if info.get("_type").and_then(|v| v.as_str()) == Some("playlist")
            || !info.get("id").map(|v| v.is_string()).unwrap_or(false)
        {
            continue;
        }
        let xml = build_nfo(&info, cfg.nfo_type);
        match fs::write(&nfo_path, xml) {
            Ok(()) => wrote += 1,
            Err(e) => eprintln!("error writing {}: {e}", nfo_path.display()),
        }
    }
    if wrote > 0 {
        println!("{}: wrote {wrote} .nfo sidecar(s)", dir.display());
    } else {
        println!("{}: no metadata to convert", dir.display());
    }
    Ok(())
}

fn youtube_id(url: &str) -> Option<String> {
    let u = Url::parse(url).ok()?;
    let host = u.host_str()?;
    if host.contains("youtu.be") {
        return u
            .path_segments()?
            .find(|s| !s.is_empty())
            .map(str::to_string);
    }
    if host.contains("youtube.com") {
        if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "v")
            && !v.is_empty()
        {
            return Some(v.into_owned());
        }
        let mut segs = u.path_segments()?.filter(|s| !s.is_empty());
        if let Some(kind) = segs.next()
            && matches!(kind, "shorts" | "live" | "embed")
        {
            return segs.next().map(str::to_string);
        }
    }
    None
}

fn find_existing_dir(base: &Path, id: &str) -> Option<PathBuf> {
    let needle = format!("[{id}]");
    let mut stack = vec![base.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(&needle))
            {
                return p.parent().map(Path::to_path_buf);
            }
        }
    }
    None
}

fn redownload_one(url: &str, cfg: &RunConfig) -> io::Result<()> {
    let is_youtube = get_domain(url)
        .map(|d| d.contains("youtube.com") || d.contains("youtu.be"))
        .unwrap_or(false);
    let dir = youtube_id(url)
        .and_then(|id| find_existing_dir(&cfg.base_dir, &id))
        .unwrap_or_else(|| cfg.base_dir.clone());
    fs::create_dir_all(&dir)?;

    let use_aria = match cfg.aria2c {
        Aria2cMode::Force => true,
        Aria2cMode::Auto => !is_youtube,
        Aria2cMode::Off => false,
    };
    let mut cmd = Command::new("yt-dlp");
    cmd.arg(url)
        .arg("--no-download-archive")
        .arg("--force-overwrites");
    add_net_flags(&mut cmd, cfg);
    add_common_flags(&mut cmd, cfg, is_youtube);
    if use_aria {
        let dl_args = if cfg.throttle {
            "aria2c:-x 2 -k 1M"
        } else {
            "aria2c:-x 4 -k 1M"
        };
        cmd.args([
            "--external-downloader",
            "aria2c",
            "--external-downloader-args",
            dl_args,
        ]);
    } else {
        let frags = if cfg.throttle { "2" } else { "4" };
        cmd.args(["--concurrent-fragments", frags]);
    }
    cmd.arg("-o").arg(dir.join("%(title)s [%(id)s].%(ext)s"));
    if !cfg.ytdlp_extra.is_empty() {
        cmd.args(&cfg.ytdlp_extra);
    }
    let status = cmd.status()?;
    println!("Re-download finished with exit status: {status}");
    generate_nfos(&dir, cfg, true)?;
    if let Err(e) = update_index(&dir, cfg) {
        eprintln!("index update failed for {}: {e}", dir.display());
    }
    Ok(())
}

fn update_index(dir: &Path, cfg: &RunConfig) -> io::Result<()> {
    let index_path = cfg.base_dir.join("downloads.tsv");
    let mut rows: std::collections::BTreeMap<String, (String, String, String)> = Default::default();
    if let Ok(text) = fs::read_to_string(&index_path) {
        for line in text.lines() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let mut it = line.splitn(4, '\t');
            let uploaded = it.next().unwrap_or("").to_string();
            let id = match it.next() {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };
            rows.insert(
                id,
                (
                    uploaded,
                    it.next().unwrap_or("").to_string(),
                    it.next().unwrap_or("").to_string(),
                ),
            );
        }
    }
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some(base) = name.strip_suffix(".info.json") else {
                continue;
            };
            let Ok(bytes) = fs::read(&p) else { continue };
            let Ok(info) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            if info.get("_type").and_then(|v| v.as_str()) == Some("playlist") {
                continue;
            }
            let Some(id) = info
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let title = info.get("title").and_then(|v| v.as_str()).unwrap_or(base);
            let uploaded = info
                .get("upload_date")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let sidecar_exts = ["nfo", "jpg", "description", "vtt", "json", "part"];
            let mut file = format!("{}/{base}.*", dir.display());
            if let Ok(sib) = fs::read_dir(dir) {
                for e in sib.flatten() {
                    let sp = e.path();
                    let Some(sname) = sp.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    if let Some(rest) = sname.strip_prefix(base) {
                        let ext = rest.trim_start_matches('.');
                        if !ext.is_empty() && !ext.contains('.') && !sidecar_exts.contains(&ext) {
                            file = format!("{}/{sname}", dir.display());
                            break;
                        }
                    }
                }
            }
            rows.insert(
                id.to_string(),
                (uploaded.to_string(), title.to_string(), file),
            );
        }
    }
    let mut out = String::from("# uploaded\tid\ttitle\tfile\n");
    let mut sorted: Vec<_> = rows.into_iter().collect();
    sorted.sort_by(|a, b| {
        a.1.1
            .to_lowercase()
            .cmp(&b.1.1.to_lowercase())
            .then(a.0.cmp(&b.0))
    });
    for (id, (uploaded, title, file)) in sorted {
        let _ = writeln!(out, "{uploaded}\t{id}\t{title}\t{file}");
    }
    fs::write(index_path, out)
}

// days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm)
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn ytdlp_age_days() -> Option<i64> {
    let out = Command::new("yt-dlp").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let s = s.trim();
    let mut it = s.split('.');
    let y: i64 = it.next()?.trim().parse().ok()?;
    let m: i64 = it.next()?.trim().parse().ok()?;
    let d: i64 = it.next()?.trim().parse().ok()?;
    let released = days_from_civil(y, m, d);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64
        / 86_400;
    Some(now - released)
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
    } else if let Some(age) = ytdlp_age_days()
        && age > 45
    {
        eprintln!(
            "Warning: yt-dlp build is {age} days old. Run with --update-ytdlp (or `yt-dlp -U`) to avoid breakage."
        );
    }

    let aria2c_available = which::which("aria2c").is_ok();
    let aria2c = if args.use_aria2c {
        Aria2cMode::Force
    } else if args.no_aria2c {
        Aria2cMode::Off
    } else if aria2c_available {
        Aria2cMode::Auto
    } else {
        Aria2cMode::Off
    };

    if args.prefer_aria2c && !aria2c_available && !args.no_aria2c {
        eprintln!(
            "aria2c not found. Install it with `sudo apt install aria2` or disable with --no-aria2c."
        );
    }

    let dir_path = "urls";
    let quality = if args.force_best_quality {
        eprintln!("note: --force-best-quality is deprecated; use --quality best");
        Quality::Best
    } else {
        args.quality
    };

    let cfg = RunConfig {
        base_dir: PathBuf::from("videos"),
        archive_file: PathBuf::from("downloaded.txt"),
        aria2c,
        quality,
        ytdlp_extra: args.ytdlp_extra,
        nfo_type: args.nfo_type,
        write_nfo: !args.no_nfo,
        force_nfo: args.force_nfo,
        refresh_metadata: args.refresh_metadata,
        throttle: args.throttle,
        cookies_from_browser: args.cookies_from_browser,
        cookies: args.cookies,
    };

    if args.refresh_metadata && !args.redownload.is_empty() {
        eprintln!("--refresh-metadata and --redownload cannot be used together");
        return Ok(());
    }
    fs::create_dir_all(&cfg.base_dir)?;

    if !args.redownload.is_empty() {
        for url in &args.redownload {
            if let Err(e) = redownload_one(url, &cfg) {
                eprintln!("error re-downloading {url}: {e}");
            }
        }
        return Ok(());
    }

    let created = create_default_structure(dir_path)?;
    if created {
        return Ok(());
    } else {
        println!(
            "Found existing {} directory. Processing .urls files...",
            dir_path
        );
    }

    let urls_exist = process_url_files(Path::new(dir_path), &cfg)?;

    if !urls_exist {
        println!(
            "No URLs found in the .urls files. Please add URLs to the .urls files for downloading videos. Each URL should be on a new line. Lines starting with '#' are considered comments and are ignored."
        );
    }

    Ok(())
}
