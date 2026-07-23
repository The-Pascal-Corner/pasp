mod batch;
mod convert;
mod error;
mod filename;

fn usage() {
    eprintln!(r#"PSP Video Toolkit v{} - Convert and batch-process videos for PSP

USAGE:
  pasp convert <input> [output]          Convert a single video for PSP
  pasp batch <input_dir> <output_dir>     Batch convert directory of videos
  pasp info <input>                       Probe video metadata
  pasp reset                              Reset batch resume state

OPTIONS:
  --width <px>       Output width  (default: 480)
  --height <px>      Output height (default: 272)
  --crf <0-51>       Quality (lower = better, default: 28)
  --maxrate <k>      Max bitrate (default: 2000k)
  --ab <k>           Audio bitrate (default: 128k)
  --preset <name>    x264 preset (default: fast)
  --pattern <str>    Output name template (default: "{{n}}_{{t}}.mp4")
  --prefix <str>     Only process files matching this prefix
  --dry-run          Preview changes without converting

EXAMPLES:
  pasp convert input.mp4 output.mp4
  pasp convert input.mkv --crf 26 --ab 96k
  pasp batch ./source ./output --prefix "Doraemon" --pattern "Doraemon Movie {{n}} - {{t}}.mp4"
  pasp info video.mp4
  pasp reset
"#, env!("CARGO_PKG_VERSION"));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "convert" => cmd_convert(&args[2..]),
        "batch" => cmd_batch(&args[2..]),
        "info" => cmd_info(&args[2..]),
        "reset" => cmd_reset(),
        "help" | "--help" | "-h" => { usage(); }
        _ => {
            eprintln!("Unknown command: {}. Use 'pasp help' for usage.", args[1]);
            std::process::exit(1);
        }
    }
}

fn parse_opts(args: &[String]) -> convert::ConvertOptions {
    let mut opts = convert::ConvertOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--width" if i + 1 < args.len() => { opts.width = args[i+1].parse().unwrap_or(480); i += 1; }
            "--height" if i + 1 < args.len() => { opts.height = args[i+1].parse().unwrap_or(272); i += 1; }
            "--crf" if i + 1 < args.len() => { opts.crf = args[i+1].parse().unwrap_or(28); i += 1; }
            "--maxrate" if i + 1 < args.len() => { opts.maxrate = args[i+1].clone(); i += 1; }
            "--ab" if i + 1 < args.len() => { opts.audio_bitrate = args[i+1].clone(); i += 1; }
            "--preset" if i + 1 < args.len() => { opts.preset = args[i+1].clone(); i += 1; }
            "--pattern" | "--prefix" | "--dry-run" => {}
            _ => {}
        }
        i += 1;
    }
    opts
}

fn cmd_convert(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: pasp convert <input> [output] [options]");
        std::process::exit(1);
    }

    let input = &args[0];
    let output = if args.len() > 1 && !args[1].starts_with("--") {
        Some(args[1].clone())
    } else {
        None
    };

    let opts = parse_opts(args);

    let out_path = output.unwrap_or_else(|| {
        let p = std::path::Path::new(input);
        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
        format!("{}_psp.mp4", stem)
    });

    match convert::convert_sync(input, &out_path, &opts) {
        Ok(()) => {
            println!("\n✓ {} → {}", input, out_path);
            match error::verify_output(input, &out_path) {
                Ok(()) => println!("✓ Output verified."),
                Err(e) => eprintln!("⚠ {}", e),
            }
        }
        Err(e) => {
            eprintln!("✗ {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_batch(args: &[String]) {
    fn inner(args: &[String]) -> Result<(), error::VideoError> {
        if args.len() < 2 {
            return Err(error::VideoError::FfmpegFailed(
                "Usage: pasp batch <input_dir> <output_dir> [options]".into(),
            ));
        }

        let input_dir = &args[0];
        let output_dir = &args[1];
        let opts = parse_opts(args);

        let pattern = args.windows(2)
            .find(|w| w[0] == "--pattern")
            .map(|w| w[1].clone())
            .unwrap_or_else(|| "{n}_{t}.mp4".to_string());

        let prefix = args.windows(2)
            .find(|w| w[0] == "--prefix")
            .map(|w| w[1].clone())
            .unwrap_or_default();

        let dry_run = args.contains(&"--dry-run".to_string());

        let resume_file = format!(".pasp-batch-{}.json", std::path::Path::new(output_dir)
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_else(|| std::borrow::Cow::Borrowed("output")));

        batch::batch_convert(input_dir, output_dir, &opts, &pattern, &prefix, &resume_file, dry_run)
    }

    match inner(args) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("✗ {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_info(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: pasp info <input>");
        std::process::exit(1);
    }

    match error::probe(&args[0]) {
        Ok(info) => {
            println!("File:      {}", info.path);
            println!("Duration:  {:.0}s ({:.1}m)", info.duration_secs, info.duration_secs / 60.0);
            println!("Size:      {}x{}", info.width, info.height);
            println!("Codec:     {}", info.codec);
            println!("Audio:     {}", info.audio_codec);
            println!("Moov atom: {}", if info.has_moov { "✓ present" } else { "✗ missing" });
        }
        Err(e) => {
            eprintln!("✗ {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_reset() {
    let dir = std::env::current_dir().unwrap_or_default();
    let mut found = false;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with(".pasp-batch-") && name.to_string_lossy().ends_with(".json") {
                let path = entry.path();
                let _ = std::fs::remove_file(&path);
                println!("Removed: {}", path.display());
                found = true;
            }
        }
    }
    if !found {
        println!("No batch state files found.");
    }
}
