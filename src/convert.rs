use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;

use crate::error::{self, VideoError};

pub fn find_ffmpeg() -> Result<String, VideoError> {
    let paths = [
        "ffmpeg.exe",
        "ffmpeg",
    ];
    for name in &paths {
        if let Ok(path) = which(name) {
            return Ok(path);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let bundled = cwd.join("ffmpeg.exe");
        if bundled.exists() {
            return Ok(bundled.to_string_lossy().to_string());
        }
    }
    Err(VideoError::FfmpegNotFound)
}

fn which(name: &str) -> Result<String, ()> {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(';') {
        let full = Path::new(dir).join(name);
        if full.exists() {
            return Ok(full.to_string_lossy().to_string());
        }
        let full_exe = Path::new(dir).join(format!("{}.exe", name));
        if full_exe.exists() {
            return Ok(full_exe.to_string_lossy().to_string());
        }
    }
    Err(())
}

pub struct ConvertOptions {
    pub width: u32,
    pub height: u32,
    pub crf: u32,
    pub maxrate: String,
    pub audio_bitrate: String,
    pub preset: String,
    pub refs: u32,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            width: 480,
            height: 272,
            crf: 28,
            maxrate: "2000k".into(),
            audio_bitrate: "128k".into(),
            preset: "fast".into(),
            refs: 2,
        }
    }
}

pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    tx: mpsc::Sender<f32>,
) -> Result<(), VideoError> {
    let ffmpeg = find_ffmpeg()?;

    if !Path::new(input).exists() {
        return Err(VideoError::MissingInput(input.to_string()));
    }

    let scale = format!(
        "scale='min({},iw)':'min({},ih)':force_original_aspect_ratio=decrease,pad={}:{}:({}-iw)/2:({}-ih)/2:color=black",
        opts.width, opts.height, opts.width, opts.height, opts.width, opts.height
    );

    let mut child = Command::new(&ffmpeg)
        .args([
            "-i", input,
            "-vf", &scale,
            "-c:v", "libx264",
            "-profile:v", "baseline",
            "-level", "3.0",
            "-pix_fmt", "yuv420p",
            "-refs", &opts.refs.to_string(),
            "-crf", &opts.crf.to_string(),
            "-maxrate", &opts.maxrate,
            "-bufsize", &opts.maxrate,
            "-preset", &opts.preset,
            "-c:a", "aac",
            "-b:a", &opts.audio_bitrate,
            "-ar", "44100",
            "-ac", "2",
            "-movflags", "+faststart",
            "-y",
            output,
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .map_err(|e| VideoError::FfmpegFailed(e.to_string()))?;

    let stderr = child.stderr.take().unwrap();
    let reader = std::io::BufReader::new(stderr);
    let mut last_time = 0.0;
    let mut total_duration = 0.0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.starts_with("  Duration: ") {
            if let Some(dur) = parse_duration(&line) {
                total_duration = dur;
            }
        }

        if line.contains("time=") {
            if let Some((secs, _speed)) = parse_progress(&line) {
                last_time = secs;
                if total_duration > 0.0 {
                    let pct = (secs as f32 / total_duration as f32).min(1.0) * 100.0;
                    let _ = tx.send(pct);
                }
            }
        }
    }

    let status = child.wait().map_err(|e| VideoError::FfmpegFailed(e.to_string()))?;
    let _ = tx.send(100.0);

    if status.success() {
        if total_duration > 0.0 && last_time > 0.0 && (last_time / total_duration) < 0.9 {
            error::verify_output(input, output)?;
        }
        Ok(())
    } else {
        Err(VideoError::FfmpegFailed("ffmpeg exited with error".into()))
    }
}

fn parse_duration(line: &str) -> Option<f64> {
    let rest = line.strip_prefix("  Duration: ")?;
    let dur_str = rest.split(',').next()?;
    let parts: Vec<&str> = dur_str.trim().split(':').collect();
    if parts.len() != 3 { return None; }
    let h: f64 = parts[0].parse().ok()?;
    let m: f64 = parts[1].parse().ok()?;
    let s: f64 = parts[2].parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

fn parse_progress(line: &str) -> Option<(f64, f64)> {
    let time_str = line.split("time=").nth(1)?;
    let time_str = time_str.split_whitespace().next()?;
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 3 { return None; }
    let h: f64 = parts[0].parse().ok()?;
    let m: f64 = parts[1].parse().ok()?;
    let s: f64 = parts[2].parse().ok()?;
    let secs = h * 3600.0 + m * 60.0 + s;

    let speed = line.split("speed=").nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.trim_end_matches('x').parse().ok())
        .unwrap_or(0.0);

    Some((secs, speed))
}

pub fn convert_sync(input: &str, output: &str, opts: &ConvertOptions) -> Result<(), VideoError> {
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        for pct in rx {
            if pct as u32 % 10 == 0 {
                print!("\rConverting... {:.0}%", pct);
                use std::io::Write;
                std::io::stdout().flush().ok();
            }
        }
    });
    let result = convert(input, output, opts, tx);
    let _ = handle.join();
    Ok(result?)
}
