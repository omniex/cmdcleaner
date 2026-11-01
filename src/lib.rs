use clap::{Parser, ValueEnum};
use colored::{Color, Colorize};
use humansize::{DECIMAL, format_size};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use walkdir::WalkDir;

const PIE_COLORS: [Color; 6] = [
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::BrightBlack,
];

fn print_pie(stats: &HashMap<String, TypeStat>, top: usize) {
    let total: u64 = stats.values().map(|s| s.total_size).sum();
    if total == 0 {
        return;
    }

    let mut sorted: Vec<(&String, &TypeStat)> = stats.iter().collect();
    sorted.sort_by(|a, b| b.1.total_size.cmp(&a.1.total_size));

    let mut slices: Vec<(String, u64)> = Vec::new();
    let mut shown: u64 = 0;

    let max_slices = (PIE_COLORS.len() - 1).min(top);

    for (name, stat) in sorted.iter().take(max_slices) {
        slices.push((name.to_string(), stat.total_size));
        shown += stat.total_size;
    }

    if shown < total {
        slices.push((String::from("other"), total - shown));
    }

    let width: usize = 60;

    let mut cumulative: u64 = 0;
    let mut prev_end: usize = 0;

    for (i, (_, size)) in slices.iter().enumerate() {
        cumulative += size;
        let end = (cumulative as f64 / total as f64 * width as f64).round() as usize;
        let len = end - prev_end;
        prev_end = end;

        let color = PIE_COLORS[i];
        print!("{}", "█".repeat(len).color(color));
    }
    println!();

    for (i, (name, size)) in slices.iter().enumerate() {
        let color = PIE_COLORS[i];
        let percent = *size as f64 / total as f64 * 100.0;

        println!(
            "{} {:>5.1}%  {:>10}  {}",
            "■".color(color),
            percent,
            format_size(*size, DECIMAL),
            name
        );
    }
    println!();
}

struct FileInfo {
    path: PathBuf,
    size: u64,
    extension: String,
}

struct TypeStat {
    count: usize,
    total_size: u64,
}

// struct KnownPlace {
//     label: &'static str,
//     path: PathBuf,
// }

fn scan(root: &Path) -> Vec<FileInfo> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template("{spinner} {pos} files scanned").unwrap());
    pb.enable_steady_tick(Duration::from_millis(100));
    let mut files = Vec::new();

    for entry in WalkDir::new(root) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let size: u64 = match entry.metadata() {
            Ok(metadata) => metadata.len(),
            Err(_) => continue,
        };

        let extension = match entry.path().extension() {
            Some(ext) => ext.to_string_lossy().to_lowercase(),
            None => "No extension".to_string(),
        };

        files.push(FileInfo {
            path: entry.path().to_path_buf(),
            size,
            extension,
        });
        pb.inc(1);
    }

    pb.finish_and_clear();

    files
}

fn calc_stats(files: &[FileInfo]) -> HashMap<String, TypeStat> {
    let mut stats: HashMap<String, TypeStat> = HashMap::new();

    for file in files {
        let stat = stats.entry(file.extension.clone()).or_insert(TypeStat {
            count: 0,
            total_size: 0,
        });

        stat.count += 1;
        stat.total_size += file.size;
    }

    stats
}

fn calc_dir_stats(files: &[FileInfo], root: &Path, depth: usize) -> HashMap<String, TypeStat> {
    let mut stats: HashMap<String, TypeStat> = HashMap::new();

    for file in files {
        let relative = match file.path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        //let mut parts = relative.components();
        // let first = parts.next();
        // let has_more = parts.next().is_some();

        let name = dir_key(relative, depth);

        let stat = stats.entry(name).or_insert(TypeStat {
            count: 0,
            total_size: 0,
        });

        stat.count += 1;
        stat.total_size += file.size;
    }

    stats
}

fn dir_key(relative: &Path, depth: usize) -> String {
    let parent = relative.parent().unwrap_or(Path::new(""));

    let parts: Vec<String> = parent
        .components()
        .take(depth)
        .map(|p| p.as_os_str().to_string_lossy().to_string())
        .collect();

    if parts.is_empty() {
        String::from("(files in root)")
    } else {
        parts.join("/")
    }
}

fn print_stats(stats: &HashMap<String, TypeStat>, top: usize, sort: &SortBy, bars: bool) {
    let value_of = |stat: &TypeStat| -> u64 {
        match sort {
            SortBy::Size => stat.total_size,
            SortBy::Count => stat.count as u64,
        }
    };

    let mut sorted: Vec<(&String, &TypeStat)> = stats.iter().collect();

    match sort {
        SortBy::Size => sorted.sort_by(|a, b| b.1.total_size.cmp(&a.1.total_size)),
        SortBy::Count => sorted.sort_by(|a, b| b.1.count.cmp(&a.1.count)),
    }

    let max_size: u64 = sorted.iter().map(|x| value_of(x.1)).max().unwrap_or(0);

    for (ext, stat) in sorted.iter().take(top) {
        let bar = if bars {
            make_bar(value_of(stat), max_size, 30)
        } else {
            String::new()
        };

        println!(
            "{:>10}  {:>6} file(s)  {:<30}  {}",
            format_size(stat.total_size, DECIMAL),
            stat.count,
            bar,
            ext
        );
    }
}

#[derive(Parser)]
#[command(version, about = "Shows what takes up disk space: biggest files, stats by extension, folder sizes. github.com/omniex")]
struct Args {
    #[arg(default_value = ".")]
    path: PathBuf,

    #[arg(short = 'n', long, default_value_t = 10)]
    top: usize,

    #[arg(long, value_enum, default_value_t = Mode::Files)]
    mode: Mode,

    #[arg(long, value_enum, default_value_t = SortBy::Size)]
    sort: SortBy,

    #[arg(long)]
    no_bars: bool,

    #[arg(long)]
    no_pie: bool,

    #[arg(long, default_value_t = 1)]
    depth: usize,
}

#[derive(Clone, ValueEnum)]
enum Mode {
    Files,
    Types,
    Dirs,
}

#[derive(Clone, ValueEnum)]
enum SortBy {
    Size,
    Count,
}

fn show_files(files: &[FileInfo], top: usize, bars: bool) {
    let mut sorted: Vec<&FileInfo> = files.iter().collect();

    sorted.sort_by(|a, b| b.size.cmp(&a.size));

    let max_size: u64 = sorted.iter().map(|f| f.size).max().unwrap_or(0);

    for file in sorted.iter().take(top) {
        let bar = if bars {
            make_bar(file.size, max_size, 30)
        } else {
            String::new()
        };

        println!(
            "{:>10}  {:<30}  {}",
            format_size(file.size, DECIMAL),
            bar,
            file.path.display(),
        );
    }
}

fn make_bar(value: u64, max: u64, width: usize) -> String {
    if max == 0 {
        return String::new();
    }

    let mut len: usize = (value as f64 / max as f64 * width as f64) as usize;

    if value > 0 && len == 0 {
        len = 1;
    }

    "█".repeat(len)
}

pub fn run() {
    let args = Args::parse();
    let files = scan(&args.path);
    println!();
    eprintln!("Found {} files", files.len());
    println!();

    let bars = !args.no_bars;
    let pie = !args.no_pie;
    match args.mode {
        Mode::Files => {
            show_files(&files, args.top, bars);
            println!();
        }
        Mode::Types => {
            let stats = calc_stats(&files);
            print_stats(&stats, args.top, &args.sort, bars);
            println!();
            if pie {
                print_pie(&stats, args.top);
            }
        }
        Mode::Dirs => {
            let stats = calc_dir_stats(&files, &args.path, args.depth);
            print_stats(&stats, args.top, &args.sort, bars);
            println!();
            if pie {
                print_pie(&stats, args.top);
            }
        }
    }
}
