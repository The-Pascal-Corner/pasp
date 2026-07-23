use std::path::Path;

#[derive(Debug)]
pub enum VideoError {
    MissingInput(String),
    FfmpegNotFound,
    FfmpegFailed(String),
    CorruptedSource(String),
    OutputTruncated { expected_secs: f64, actual_secs: f64, path: String },
    Io(std::io::Error),
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoError::MissingInput(p) => write!(f, "Input file not found: {}", p),
            VideoError::FfmpegNotFound => write!(f, "ffmpeg not found in PATH. Install ffmpeg or place ffmpeg.exe alongside pasp.exe"),
            VideoError::FfmpegFailed(msg) => write!(f, "ffmpeg failed: {}", msg),
            VideoError::CorruptedSource(p) => write!(f, "Source file may be corrupted: {}", p),
            VideoError::OutputTruncated { expected_secs, actual_secs, path } => {
                write!(f, "Output truncated: expected {:.0}s, got {:.0}s — {}", expected_secs, actual_secs, path)
            }
            VideoError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for VideoError {}

impl From<std::io::Error> for VideoError {
    fn from(e: std::io::Error) -> Self { VideoError::Io(e) }
}

pub struct VideoInfo {
    pub path: String,
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub audio_codec: String,
    pub has_moov: bool,
}

impl std::fmt::Display for VideoInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} — {}x{}, {} ({:.0}s)", self.path, self.width, self.height, self.codec, self.duration_secs)
    }
}

pub fn probe(path: &str) -> Result<VideoInfo, VideoError> {
    if !Path::new(path).exists() {
        return Err(VideoError::MissingInput(path.to_string()));
    }

    let probe_output = run_ffprobe(path)?;
    parse_ffprobe_output(&probe_output, path)
}

fn run_ffprobe(path: &str) -> Result<String, VideoError> {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration,format_name",
            "-show_entries", "stream=codec_name,codec_type,width,height",
            "-of", "csv=p=0",
            path,
        ])
        .output()
        .map_err(|_| VideoError::FfmpegNotFound)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("moov atom not found") {
            return Err(VideoError::CorruptedSource(path.to_string()));
        }
        return Err(VideoError::FfmpegFailed(stderr.to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_ffprobe_output(output: &str, path: &str) -> Result<VideoInfo, VideoError> {
    let mut duration = 0.0;
    let mut width = 0;
    let mut height = 0;
    let mut codec = String::new();
    let mut audio_codec = String::new();
    let has_moov = !output.contains("moov atom not found");

    for line in output.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.is_empty() { continue; }
        if parts.len() >= 3 && (parts[1] == "video" || parts[1] == "\"video\"") {
            codec = parts[0].trim_matches('"').to_string();
            if parts.len() >= 4 { width = parts[2].trim_matches('"').parse().unwrap_or(0); }
            if parts.len() >= 5 { height = parts[3].trim_matches('"').parse().unwrap_or(0); }
        }
        if parts.len() >= 2 && (parts[1] == "audio" || parts[1] == "\"audio\"") {
            audio_codec = parts[0].trim_matches('"').to_string();
        }
        if parts.len() == 2 && parts[0].trim_matches('"') == "duration" {
            duration = parts[1].trim_matches('"').parse().unwrap_or(0.0);
        }
    }

    if duration == 0.0 && !output.is_empty() {
        let dur_line = output.lines().find(|l| l.starts_with("duration,"));
        if let Some(line) = dur_line {
            let val = line.split(',').nth(1).unwrap_or("0");
            duration = val.trim_matches('"').parse().unwrap_or(0.0);
        }
    }

    Ok(VideoInfo {
        path: path.to_string(),
        duration_secs: duration,
        width,
        height,
        codec,
        audio_codec,
        has_moov,
    })
}

pub fn verify_output(input_path: &str, output_path: &str) -> Result<(), VideoError> {
    let input_info = probe(input_path)?;
    let output_info = probe(output_path)?;

    if !output_info.has_moov {
        return Err(VideoError::CorruptedSource(output_path.to_string()));
    }

    if output_info.duration_secs > 0.0 && input_info.duration_secs > 0.0 {
        let ratio = output_info.duration_secs / input_info.duration_secs;
        if ratio < 0.9 {
            return Err(VideoError::OutputTruncated {
                expected_secs: input_info.duration_secs,
                actual_secs: output_info.duration_secs,
                path: output_path.to_string(),
            });
        }
    }

    Ok(())
}
