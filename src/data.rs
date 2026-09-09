use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{Datelike, Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entry {
    pub date: NaiveDate,
    pub weight: f32,
}

#[derive(Serialize)]
pub struct WeekSummary {
    pub week: String,
    pub min: f32,
    pub delta_start: f32,
    pub delta_week: Option<f32>,
}

#[derive(Serialize)]
pub struct Report {
    pub entries: Vec<Entry>,
    pub weekly: Vec<WeekSummary>,
    pub start_weight: Option<f32>,
    pub start_date: Option<String>,
    pub current_weight: Option<f32>,
    pub current_date: Option<String>,
    pub total_lost: Option<f32>,
    pub rate: Option<f32>,
    pub goal: Option<f32>,
    pub goal_date: Option<String>,
}

pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".local/share/fitness");
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn data_path() -> PathBuf {
    data_dir().join("weights.json")
}

pub fn goal_path() -> PathBuf {
    data_dir().join("goal.txt")
}

pub fn load() -> Vec<Entry> {
    let path = data_path();
    if !path.exists() {
        return vec![];
    }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(entries: &[Entry]) {
    std::fs::write(data_path(), serde_json::to_string_pretty(entries).unwrap()).unwrap();
}

pub fn load_goal() -> Option<f32> {
    std::fs::read_to_string(goal_path())
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn save_goal(weight: f32) {
    std::fs::write(goal_path(), weight.to_string()).unwrap();
}

pub fn week_monday(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

pub fn build_report(entries: Vec<Entry>, goal: Option<f32>) -> Report {
    if entries.is_empty() {
        return Report {
            entries,
            weekly: vec![],
            start_weight: None,
            start_date: None,
            current_weight: None,
            current_date: None,
            total_lost: None,
            rate: None,
            goal,
            goal_date: None,
        };
    }

    let mut by_week: BTreeMap<NaiveDate, Vec<f32>> = BTreeMap::new();
    for e in &entries {
        by_week.entry(week_monday(e.date)).or_default().push(e.weight);
    }

    let start_weight = entries.first().unwrap().weight;
    let start_date = entries.first().unwrap().date.to_string();
    let last_weight = entries.last().unwrap().weight;
    let last_date = entries.last().unwrap().date.to_string();

    let mut weekly = vec![];
    let mut prev_min: Option<f32> = None;
    for (week, weights) in &by_week {
        let min = weights.iter().cloned().fold(f32::INFINITY, f32::min);
        weekly.push(WeekSummary {
            week: week.to_string(),
            min,
            delta_start: min - start_weight,
            delta_week: prev_min.map(|p| min - p),
        });
        prev_min = Some(min);
    }

    let current_min = by_week
        .values()
        .last()
        .and_then(|w| w.iter().cloned().reduce(f32::min))
        .unwrap_or(last_weight);

    let total_lost = current_min - start_weight;
    let rate = total_lost / by_week.len() as f32;

    let goal_date = goal.filter(|_| rate < 0.0).map(|g| {
        let weeks = (g - current_min) / rate;
        (Local::now().date_naive() + Duration::days((weeks * 7.0) as i64)).to_string()
    });

    Report {
        entries,
        weekly,
        start_weight: Some(start_weight),
        start_date: Some(start_date),
        current_weight: Some(last_weight),
        current_date: Some(last_date),
        total_lost: Some(total_lost),
        rate: Some(rate),
        goal,
        goal_date,
    }
}
