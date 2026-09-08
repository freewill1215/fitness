use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{Datelike, Duration, Local, NaiveDate};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "fitness", about = "Weight tracking CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Log a weight measurement (default date: today)
    Log {
        weight: f32,
        /// Date in YYYY-MM-DD format
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Show weekly progress report
    Report {
        /// Number of weeks to display
        #[arg(short, long, default_value = "12")]
        weeks: usize,
    },
    /// Import historical weights from a Numbers/Excel spreadsheet
    Import {
        file: String,
    },
    /// Set or show the goal weight
    Goal {
        /// Target weight in lbs (omit to show current goal)
        weight: Option<f32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Entry {
    date: NaiveDate,
    weight: f32,
}

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".local/share/fitness");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn data_path() -> PathBuf {
    data_dir().join("weights.json")
}

fn goal_path() -> PathBuf {
    data_dir().join("goal.txt")
}

fn load_goal() -> Option<f32> {
    std::fs::read_to_string(goal_path())
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

fn save_goal(weight: f32) {
    std::fs::write(goal_path(), weight.to_string()).unwrap();
}

fn cmd_goal(weight: Option<f32>) {
    match weight {
        Some(w) => {
            save_goal(w);
            println!("Goal set to {:.1} lbs", w);
        }
        None => match load_goal() {
            Some(g) => println!("Current goal: {:.1} lbs", g),
            None => println!("No goal set. Use `fitness goal <weight>` to set one."),
        },
    }
}

fn load() -> Vec<Entry> {
    let path = data_path();
    if !path.exists() {
        return vec![];
    }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

fn save(entries: &[Entry]) {
    let path = data_path();
    std::fs::write(path, serde_json::to_string_pretty(entries).unwrap()).unwrap();
}

fn week_monday(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

fn excel_serial_to_date(serial: i64) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(1899, 12, 30).map(|epoch| epoch + Duration::days(serial))
}

fn cmd_log(weight: f32, date: Option<String>, entries: &mut Vec<Entry>) {
    let date = match date {
        Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d").expect("Date must be YYYY-MM-DD"),
        None => Local::now().date_naive(),
    };

    if let Some(e) = entries.iter_mut().find(|e| e.date == date) {
        println!("Updated {}: {:.1} → {:.1} lbs", date, e.weight, weight);
        e.weight = weight;
    } else {
        println!("Logged {:.1} lbs on {}", weight, date);
        entries.push(Entry { date, weight });
        entries.sort_by_key(|e| e.date);
    }
}

fn cmd_report(weeks: usize, entries: &[Entry], goal: Option<f32>) {
    if entries.is_empty() {
        println!("No entries yet. Use `fitness log <weight>` to start.");
        return;
    }

    let mut by_week: BTreeMap<NaiveDate, Vec<f32>> = BTreeMap::new();
    for e in entries {
        by_week.entry(week_monday(e.date)).or_default().push(e.weight);
    }

    let start_weight = entries.first().unwrap().weight;
    let start_date = entries.first().unwrap().date;
    let last_entry = entries.last().unwrap();

    let all_weeks: Vec<_> = by_week.iter().collect();
    let show_from = all_weeks.len().saturating_sub(weeks);
    let visible = &all_weeks[show_from..];

    println!("{:<12} {:>7} {:>8} {:>8}", "Week of", "Min", "Δ Start", "Δ Week");
    println!("{}", "─".repeat(40));

    let mut prev_min: Option<f32> = None;
    for (week, weights) in visible {
        let min = weights.iter().cloned().fold(f32::INFINITY, f32::min);
        let d_start = format!("{:+.1}", min - start_weight);
        let d_week = prev_min
            .map(|p| format!("{:+.1}", min - p))
            .unwrap_or_else(|| "  —".to_string());
        println!("{:<12} {:>7.1} {:>8} {:>8}", week, min, d_start, d_week);
        prev_min = Some(min);
    }

    println!();

    let current_min = by_week
        .values()
        .last()
        .and_then(|w| w.iter().cloned().reduce(f32::min))
        .unwrap_or(last_entry.weight);

    let total_lost = current_min - start_weight;
    let num_weeks = by_week.len() as f32;
    let rate = total_lost / num_weeks;

    println!("Start:    {:.1} lbs  ({})", start_weight, start_date);
    println!("Current:  {:.1} lbs  ({})", last_entry.weight, last_entry.date);
    println!(
        "Lost:     {:+.1} lbs over {} weeks  ({:.2} lbs/week)",
        total_lost,
        by_week.len(),
        rate
    );

    if let Some(g) = goal {
        let remaining = g - current_min;
        println!("Goal:     {:.1} lbs  ({:+.1} to go)", g, remaining);
        if rate < 0.0 && remaining < 0.0 {
            let weeks_remaining = remaining / rate;
            let goal_date =
                Local::now().date_naive() + Duration::days((weeks_remaining * 7.0) as i64);
            println!("At this rate: goal by ~{}", goal_date);
        }
    } else if rate < 0.0 {
        println!("(Set a goal with `fitness goal <weight>`)");
    }
}

fn cmd_import(file: &str, entries: &mut Vec<Entry>) {
    use calamine::{open_workbook, DataType, Reader, Xlsx};

    let mut wb: Xlsx<_> = open_workbook(file).expect("Cannot open file");
    let range = wb
        .worksheet_range("Weight")
        .expect("No 'Weight' sheet found");

    // Col 1 = Excel serial date = Saturday of that tracking week
    // Cols 6–12 = daily weights: Sun(−6), Mon(−5), Tue(−4), Wed(−3), Thu(−2), Fri(−1), Sat(0)
    let day_cols: [(usize, i64); 7] = [
        (6, -6),
        (7, -5),
        (8, -4),
        (9, -3),
        (10, -2),
        (11, -1),
        (12, 0),
    ];

    let mut imported = 0;
    let mut skipped = 0;

    for row in range.rows().skip(1) {
        let serial = match row.get(1) {
            Some(calamine::Data::DateTime(dt)) => dt.as_f64() as i64,
            Some(other) => match other.as_f64() {
                Some(f) => f as i64,
                None => continue,
            },
            None => continue,
        };
        let week_sat = match excel_serial_to_date(serial) {
            Some(d) => d,
            None => continue,
        };

        for (col, offset) in day_cols {
            let w = match row.get(col).and_then(|c| c.as_f64()) {
                Some(f) if f > 100.0 && f < 500.0 => f as f32,
                _ => continue,
            };

            let date = week_sat + Duration::days(offset);
            if entries.iter().any(|e| e.date == date) {
                skipped += 1;
                continue;
            }
            entries.push(Entry { date, weight: w });
            imported += 1;
        }
    }

    entries.sort_by_key(|e| e.date);
    println!("Imported {} entries ({} skipped as duplicates)", imported, skipped);
}

fn main() {
    let cli = Cli::parse();
    let mut entries = load();

    match cli.command {
        Commands::Log { weight, date } => {
            cmd_log(weight, date, &mut entries);
            save(&entries);
        }
        Commands::Report { weeks } => {
            cmd_report(weeks, &entries, load_goal());
        }
        Commands::Import { file } => {
            cmd_import(&file, &mut entries);
            save(&entries);
        }
        Commands::Goal { weight } => {
            cmd_goal(weight);
        }
    }
}
