use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn command_exists(cmd: &str) -> bool {
    let output = if cfg!(target_os = "windows") {
        Command::new("where").arg("/Q").arg(cmd).output()
    } else {
        Command::new("which").arg(cmd).output()
    };

    output.map_or(false, |o| o.status.success())
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

fn process_url_files(dir_path: &str, base_dir: &str, archive_file: &str) -> io::Result<bool> {
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
                if !url.starts_with('#') {
                    urls_exist = true;
                    break;
                }
            }

            if urls_exist {
                let status = Command::new("yt-dlp")
                    .arg("-a")
                    .arg(path.to_str().unwrap())
                    .arg("--download-archive")
                    .arg(archive_file)
                    .arg("-f")
                    .arg("bestvideo+bestaudio")
                    .arg("--prefer-ffmpeg")
                    .arg("--write-description")
                    .arg("--add-metadata")
                    .arg("--write-auto-sub")
                    .arg("--embed-subs")
                    .arg("-o")
                    .arg(output_dir.join("%(title)s.%(ext)s").to_str().unwrap())
                    .status()?;

                println!("Download finished with exit status: {}", status);
            }
        }
    }

    Ok(urls_exist)
}

fn main() -> io::Result<()> {
    if !check_dependencies() {
        return Ok(());
    }

    let dir_path = "urls";
    let base_dir = "videos";
    let archive_file = "downloaded.txt";

    if create_default_structure(dir_path)? {
        return Ok(());
    }

    let urls_exist = process_url_files(dir_path, base_dir, archive_file)?;

    if !urls_exist {
        println!("No URLs found in the .urls files. Please add URLs to the .urls files for downloading videos. Each URL should be on a new line. Lines starting with '#' are considered comments and are ignored.");
    }

    Ok(())
}
