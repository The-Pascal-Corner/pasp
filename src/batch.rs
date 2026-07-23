use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::convert::{self, ConvertOptions};
use crate::error::{self, VideoError};
use crate::filename;

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchState {
    pub completed: HashSet<String>,
    pub failed: HashSet<String>,
    pub skipped: HashSet<String>,
}

impl BatchState {
    pub fn load(path: &str) -> Self {
        let content = fs::read_to_string(path).ok();
        content
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_else(|| BatchState {
                completed: HashSet::new(),
                failed: HashSet::new(),
                skipped: HashSet::new(),
            })
    }

    pub fn save(&self, path: &str) -> Result<(), VideoError> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }
}

impl From<serde_json::Error> for VideoError {
    fn from(e: serde_json::Error) -> Self {
        VideoError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }
}

pub fn batch_convert(
    input_dir: &str,
    output_dir: &str,
    opts: &ConvertOptions,
    pattern: &str,
    prefix: &str,
    resume_file: &str,
    dry_run: bool,
) -> Result<(), VideoError> {
    let input_path = Path::new(input_dir);
    let output_path = Path::new(output_dir);

    if !input_path.exists() {
        return Err(VideoError::MissingInput(input_dir.to_string()));
    }
    fs::create_dir_all(output_path)?;

    let mut state = BatchState::load(resume_file);

    let ext_filter = ["mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "m4v"];

    let mut entries: Vec<_> = fs::read_dir(input_path)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file()
                && ext_filter.iter().any(|ext| {
                    e.path()
                        .extension()
                        .map(|x| x.eq_ignore_ascii_case(ext))
                        .unwrap_or(false)
                })
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let total = entries.len();
    let mut processed = 0;
    let mut ok_count = 0;
    let mut fail_count = 0;
    let mut skip_count = 0;

    for entry in &entries {
        let in_name = entry.file_name();
        let in_name_str = in_name.to_string_lossy().to_string();
        processed += 1;

        print!("\n[{}/{}] {}", processed, total, in_name_str);

        if state.completed.contains(&in_name_str) {
            println!(" ✓ (cached)");
            ok_count += 1;
            continue;
        }
        if state.failed.contains(&in_name_str) {
            println!(" ✗ (cached fail)");
            fail_count += 1;
            continue;
        }
        if state.skipped.contains(&in_name_str) {
            println!(" - (skipped)");
            skip_count += 1;
            continue;
        }

        if !prefix.is_empty() && !in_name_str.to_lowercase().contains(&prefix.to_lowercase()) {
            state.skipped.insert(in_name_str.clone());
            println!(" - (no match)");
            skip_count += 1;
            state.save(resume_file)?;
            continue;
        }

        let num = filename::parse_number(&in_name_str);
        let raw_title = filename::extract_title(&in_name_str, prefix).unwrap_or_default();
        let clean_title = filename::remove_diacritics(&raw_title);
        let out_name = filename::format_name(pattern, num.unwrap_or(processed as u32), &clean_title);
        let out_path = output_path.join(&out_name);

        if out_path.exists() {
            match error::verify_output(entry.path().to_str().unwrap(), out_path.to_str().unwrap()) {
                Ok(_) => {
                    state.completed.insert(in_name_str.clone());
                    println!(" ✓ (exists, verified)");
                    ok_count += 1;
                    state.save(resume_file)?;
                    continue;
                }
                Err(_) => {
                    println!("\n  ⚠ Output exists but appears invalid, re-converting...");
                    let _ = fs::remove_file(&out_path);
                }
            }
        }

        if dry_run {
            println!("\n  → {}", out_name);
            continue;
        }

        let input_str = entry.path().to_string_lossy().to_string();
        let output_str = out_path.to_string_lossy().to_string();

        match convert::convert_sync(&input_str, &output_str, opts) {
            Ok(()) => {
                match error::verify_output(&input_str, &output_str) {
                    Ok(()) => {
                        state.completed.insert(in_name_str.clone());
                        println!(" ✓");
                        ok_count += 1;
                    }
                    Err(e) => {
                        state.failed.insert(in_name_str.clone());
                        println!(" ✗ verify: {}", e);
                        fail_count += 1;
                    }
                }
            }
            Err(e) => {
                state.failed.insert(in_name_str.clone());
                println!(" ✗ {}", e);
                fail_count += 1;
            }
        }

        state.save(resume_file)?;
    }

    println!(
        "\nDone. {}/{} OK, {}/{} fail, {}/{} skipped.",
        ok_count, total, fail_count, total, skip_count, total
    );

    Ok(())
}
