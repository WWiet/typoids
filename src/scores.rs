//! The arcade high-score table and the daily-run best, stored as plain
//! `key=value` lines in `~/.typoids-scores`. Pure data plus `encode`/`decode`.

use std::path::PathBuf;

/// Entries the table keeps.
pub const KEEP: usize = 5;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Entry {
    /// Three capital letters (padded with `-`).
    pub initials: String,
    pub score: u32,
    pub wave: u32,
    /// The run beat the final boss.
    pub won: bool,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Scores {
    /// Best first.
    pub entries: Vec<Entry>,
    /// Best score of a daily run: (day number, score).
    pub daily: Option<(u32, u32)>,
}

impl Scores {
    /// Would `score` earn a place on the table?
    pub fn qualifies(&self, score: u32) -> bool {
        score > 0
            && (self.entries.len() < KEEP || self.entries.last().is_some_and(|e| score > e.score))
    }

    /// Insert an entry in order and drop anything past `KEEP`. Returns its
    /// rank (0 = top) if it stayed on the table.
    pub fn insert(&mut self, entry: Entry) -> Option<usize> {
        let at = self
            .entries
            .iter()
            .position(|e| entry.score > e.score)
            .unwrap_or(self.entries.len());
        if at >= KEEP {
            return None;
        }
        self.entries.insert(at, entry);
        self.entries.truncate(KEEP);
        Some(at)
    }

    /// Record a daily-run score. A new day replaces an old day's best.
    pub fn record_daily(&mut self, day: u32, score: u32) -> bool {
        match self.daily {
            Some((d, best)) if d == day && best >= score => false,
            _ => {
                self.daily = Some((day, score));
                true
            }
        }
    }

    /// Today's daily best, if one was set today.
    pub fn daily_best(&self, day: u32) -> Option<u32> {
        self.daily.filter(|&(d, _)| d == day).map(|(_, s)| s)
    }
}

/// Uppercase letters only, padded or cut to three.
pub fn clean_initials(text: &str) -> String {
    let mut out: String = text
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .take(3)
        .collect();
    while out.len() < 3 {
        out.push('-');
    }
    out
}

pub fn encode(scores: &Scores) -> String {
    let mut out = String::new();
    for e in &scores.entries {
        out.push_str(&format!(
            "entry={},{},{},{}\n",
            e.initials, e.score, e.wave, e.won as u8
        ));
    }
    if let Some((day, score)) = scores.daily {
        out.push_str(&format!("daily={day},{score}\n"));
    }
    out
}

/// Bad lines are skipped; the table is re-sorted and trimmed.
pub fn decode(text: &str) -> Scores {
    let mut scores = Scores::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let parts: Vec<&str> = value.trim().split(',').collect();
        match (key.trim(), parts.as_slice()) {
            ("entry", [name, score, wave, won]) => {
                let (Ok(score), Ok(wave), Ok(won)) =
                    (score.parse(), wave.parse(), won.parse::<u8>())
                else {
                    continue;
                };
                scores.insert(Entry {
                    initials: clean_initials(name),
                    score,
                    wave,
                    won: won != 0,
                });
            }
            ("daily", [day, score]) => {
                if let (Ok(day), Ok(score)) = (day.parse(), score.parse()) {
                    scores.daily = Some((day, score));
                }
            }
            _ => {}
        }
    }
    scores
}

pub fn path() -> Option<PathBuf> {
    crate::save::home_file(".typoids-scores")
}

/// `YYYY-MM-DD` for a day number counted from 1970-01-01.
pub fn date_label(day: u32) -> String {
    // Howard Hinnant's civil-from-days.
    let z = day as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, score: u32) -> Entry {
        Entry {
            initials: clean_initials(name),
            score,
            wave: 3,
            won: false,
        }
    }

    #[test]
    fn the_table_keeps_the_best_five_in_order() {
        let mut s = Scores::default();
        for (i, score) in [300, 900, 100, 500, 700, 200].into_iter().enumerate() {
            s.insert(entry(&format!("a{i}"), score));
        }
        let got: Vec<u32> = s.entries.iter().map(|e| e.score).collect();
        assert_eq!(got, [900, 700, 500, 300, 200]);
        assert!(!s.qualifies(150));
        assert!(s.qualifies(250));
        assert!(!s.qualifies(0));
    }

    #[test]
    fn scores_survive_a_round_trip() {
        let mut s = Scores::default();
        s.insert(entry("abc", 1234));
        s.insert(Entry {
            won: true,
            ..entry("zz", 99)
        });
        s.record_daily(20_396, 4321);
        assert_eq!(decode(&encode(&s)), s);
        assert_eq!(s.entries[1].initials, "ZZ-");
    }

    #[test]
    fn a_new_day_replaces_the_daily_best() {
        let mut s = Scores::default();
        assert!(s.record_daily(10, 500));
        assert!(!s.record_daily(10, 400));
        assert!(s.record_daily(11, 100));
        assert_eq!(s.daily_best(11), Some(100));
        assert_eq!(s.daily_best(10), None);
    }

    #[test]
    fn day_numbers_become_dates() {
        assert_eq!(date_label(0), "1970-01-01");
        assert_eq!(date_label(20_396), "2025-11-04");
        assert_eq!(date_label(20_761), "2026-11-04");
    }
}
