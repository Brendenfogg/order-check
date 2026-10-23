//! Anonymous per-order records and the restaurant dashboard built from them.
//! A record holds counts and menu item ids only: no transcript, no employee.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

use serde::{Deserialize, Serialize};

use crate::menu::Menu;
use crate::verify::{MismatchKind, Outcome};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderRecord {
    pub order_num: u32,
    pub mode: String,
    pub ts: i64,
    pub outcome: Outcome,
    pub caught: u32,
    pub kinds: Vec<MismatchKind>,
    pub pairs: Vec<(Option<String>, Option<String>)>,
    pub fixed: bool,
    pub sent_as_rung: bool,
    pub standards: Option<BTreeMap<String, bool>>,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Dashboard {
    pub orders: u32,
    pub orders_with_catch: u32,
    pub caught_total: u32,
    pub fix_rate: f32,
    pub by_kind: BTreeMap<String, u32>,
    pub by_hour: Vec<(u32, u32, u32)>,
    pub top_pairs: Vec<(String, String, u32)>,
    pub standards_rates: BTreeMap<String, f32>,
    pub p50_latency_ms: u32,
    pub demo_data: bool,
}

pub struct Records {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY,
    order_num INTEGER NOT NULL,
    mode TEXT NOT NULL,
    ts INTEGER NOT NULL,
    outcome TEXT NOT NULL,
    caught INTEGER NOT NULL,
    kinds TEXT NOT NULL,
    pairs TEXT NOT NULL,
    fixed INTEGER NOT NULL,
    sent_as_rung INTEGER NOT NULL,
    standards TEXT,
    latency_ms INTEGER NOT NULL
)";

impl Records {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> anyhow::Result<Self> {
        conn.execute(SCHEMA, [])?;
        Ok(Records {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn insert(&self, r: &OrderRecord) -> anyhow::Result<()> {
        self.conn().execute(
            "INSERT INTO orders (order_num, mode, ts, outcome, caught, kinds, pairs, fixed, sent_as_rung, standards, latency_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                r.order_num,
                r.mode,
                r.ts,
                serde_json::to_string(&r.outcome)?,
                r.caught,
                serde_json::to_string(&r.kinds)?,
                serde_json::to_string(&r.pairs)?,
                r.fixed,
                r.sent_as_rung,
                r.standards.as_ref().map(serde_json::to_string).transpose()?,
                r.latency_ms,
            ],
        )?;
        Ok(())
    }

    fn load(&self, since_ts: i64) -> anyhow::Result<Vec<(OrderRecord, u32)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT order_num, mode, ts, outcome, caught, kinds, pairs, fixed, sent_as_rung, standards, latency_ms,
                    CAST(strftime('%H', ts, 'unixepoch', 'localtime') AS INTEGER)
             FROM orders WHERE ts >= ?1 ORDER BY ts",
        )?;
        let rows = stmt.query_map([since_ts], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, bool>(7)?,
                row.get::<_, bool>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, u32>(10)?,
                row.get::<_, u32>(11)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (
                order_num,
                mode,
                ts,
                outcome,
                caught,
                kinds,
                pairs,
                fixed,
                sent_as_rung,
                standards,
                latency_ms,
                hour,
            ) = row?;
            out.push((
                OrderRecord {
                    order_num,
                    mode,
                    ts,
                    outcome: serde_json::from_str(&outcome)?,
                    caught,
                    kinds: serde_json::from_str(&kinds)?,
                    pairs: serde_json::from_str(&pairs)?,
                    fixed,
                    sent_as_rung,
                    standards: standards.map(|s| serde_json::from_str(&s)).transpose()?,
                    latency_ms,
                },
                hour,
            ));
        }
        Ok(out)
    }

    pub fn dashboard(&self, menu: &Menu, since_ts: i64) -> anyhow::Result<Dashboard> {
        let rows = self.load(since_ts)?;
        let name = |id: &str| {
            menu.item(id)
                .map(|i| i.name.clone())
                .unwrap_or_else(|| id.to_string())
        };
        let mut by_kind: BTreeMap<String, u32> = BTreeMap::new();
        let mut by_hour: BTreeMap<u32, (u32, u32)> = BTreeMap::new();
        let mut pairs: BTreeMap<(String, String), u32> = BTreeMap::new();
        let mut standards: BTreeMap<String, (u32, u32)> = BTreeMap::new();
        let mut latencies = Vec::new();
        let (mut with_catch, mut caught_total, mut fixed) = (0, 0, 0);
        for (r, hour) in &rows {
            let h = by_hour.entry(*hour).or_default();
            h.0 += 1;
            h.1 += r.caught;
            caught_total += r.caught;
            if r.caught > 0 {
                with_catch += 1;
            }
            if r.fixed {
                fixed += 1;
            }
            for k in &r.kinds {
                *by_kind
                    .entry(
                        serde_json::to_value(k)?
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                    )
                    .or_default() += 1;
            }
            for (said, rang) in &r.pairs {
                // Only look-alike mix-ups; a wrong count of the right item isn't one.
                if let (Some(s), Some(g)) = (said, rang)
                    && s != g
                {
                    *pairs.entry((name(s), name(g))).or_default() += 1;
                }
            }
            for (k, v) in r.standards.iter().flatten() {
                let e = standards.entry(k.clone()).or_default();
                e.0 += *v as u32;
                e.1 += 1;
            }
            latencies.push(r.latency_ms);
        }
        latencies.sort_unstable();
        let mut top_pairs: Vec<(String, String, u32)> =
            pairs.into_iter().map(|((s, g), n)| (s, g, n)).collect();
        top_pairs.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        top_pairs.truncate(10);
        Ok(Dashboard {
            orders: rows.len() as u32,
            orders_with_catch: with_catch,
            caught_total,
            fix_rate: if with_catch == 0 {
                0.0
            } else {
                fixed as f32 / with_catch as f32
            },
            by_kind,
            by_hour: by_hour.into_iter().map(|(h, (o, c))| (h, o, c)).collect(),
            top_pairs,
            standards_rates: standards
                .into_iter()
                .map(|(k, (yes, n))| (k, yes as f32 / n as f32))
                .collect(),
            p50_latency_ms: latencies.get(latencies.len() / 2).copied().unwrap_or(0),
            demo_data: rows.iter().any(|(r, _)| r.mode == "demo"),
        })
    }

    pub fn column_names(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('orders') ORDER BY cid")?;
        let names = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(names)
    }
}

/// Fills the store with plausible synthetic records marked `mode = "demo"`,
/// so the dashboard can be shown before any real orders exist.
pub fn seed_demo(records: &Records, menu: &Menu, n: u32) -> anyhow::Result<()> {
    // Look-alike mix-ups seen at the counter: (said, rang).
    const VARIANTS: &[(&str, &str)] = &[
        ("spicy_sandwich", "chicken_sandwich"),
        ("nuggets_12", "nuggets_8"),
        ("deluxe_sandwich", "chicken_sandwich"),
        ("unsweet_tea", "sweet_tea"),
        ("diet_lemonade", "lemonade"),
        ("spicy_deluxe", "deluxe_sandwich"),
        ("grilled_nuggets_8", "nuggets_8"),
    ];
    const SAUCES: &[&str] = &[
        "polynesian",
        "cfa_sauce",
        "honey_mustard",
        "garden_herb_ranch",
    ];
    let variants: Vec<_> = VARIANTS
        .iter()
        .filter(|(a, b)| menu.item(a).is_some() && menu.item(b).is_some())
        .collect();
    let sauces: Vec<_> = SAUCES.iter().filter(|s| menu.item(s).is_some()).collect();

    // Small deterministic generator so the demo looks the same every time.
    let mut state: u64 = 0x5eed_0c0f_fee5;
    let mut rand = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % 10_000) as f32 / 10_000.0
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Seed in the restaurant's local hours, since the dashboard shows local time.
    let offset: i64 = records.conn().query_row(
        "SELECT CAST(strftime('%s', 'now', 'localtime') AS INTEGER) - CAST(strftime('%s', 'now') AS INTEGER)",
        [],
        |r| r.get(0),
    )?;
    let local_now = now + offset;
    let day_start = local_now - local_now.rem_euclid(86_400) - offset;
    // Hours weighted toward lunch and dinner rushes.
    const HOURS: &[(i64, f32)] = &[
        (7, 0.4),
        (8, 0.6),
        (9, 0.4),
        (10, 0.5),
        (11, 1.4),
        (12, 2.0),
        (13, 1.3),
        (14, 0.6),
        (15, 0.5),
        (16, 0.7),
        (17, 1.3),
        (18, 1.6),
        (19, 1.0),
        (20, 0.5),
    ];
    let total_w: f32 = HOURS.iter().map(|h| h.1).sum();

    for i in 0..n {
        let day = (rand() * 7.0) as i64;
        let mut pick = rand() * total_w;
        let hour = HOURS
            .iter()
            .find(|h| {
                pick -= h.1;
                pick <= 0.0
            })
            .map_or(12, |h| h.0);
        let ts = (day_start - day * 86_400 + hour * 3600 + (rand() * 3599.0) as i64).min(now - 60);

        let caught_order = rand() < 0.18;
        let (mut kinds, mut pairs) = (Vec::new(), Vec::new());
        if caught_order {
            let r = rand();
            if r < 0.35 && !sauces.is_empty() {
                let s = sauces[(rand() * sauces.len() as f32) as usize % sauces.len()];
                kinds.push(MismatchKind::Count);
                pairs.push((Some(s.to_string()), Some(s.to_string())));
            } else if r < 0.70 && !variants.is_empty() {
                let (a, b) = variants[(rand() * variants.len() as f32) as usize % variants.len()];
                kinds.push(MismatchKind::Variant);
                pairs.push((Some(a.to_string()), Some(b.to_string())));
            } else if r < 0.85 {
                kinds.push(MismatchKind::Size);
                pairs.push((Some("lemonade".into()), Some("lemonade".into())));
            } else if r < 0.96 {
                kinds.push(MismatchKind::Modifier);
                pairs.push((
                    Some("chicken_sandwich".into()),
                    Some("chicken_sandwich".into()),
                ));
            } else {
                kinds.push(MismatchKind::Missing);
                pairs.push((Some("cookie".into()), None));
            }
        }
        let fixed = caught_order && rand() < 0.86;
        records.insert(&OrderRecord {
            order_num: 1001 + i,
            mode: "demo".into(),
            ts,
            outcome: if caught_order && !fixed {
                Outcome::Mismatch
            } else {
                Outcome::Match
            },
            caught: kinds.len() as u32,
            kinds,
            pairs,
            fixed,
            sent_as_rung: caught_order && !fixed,
            standards: Some(BTreeMap::from([
                ("my_pleasure".to_string(), rand() < 0.88),
                ("offered_lto".to_string(), rand() < 0.61),
            ])),
            latency_ms: 280 + (rand() * 520.0) as u32,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::cfa;

    fn rec(
        n: u32,
        caught: u32,
        kinds: Vec<MismatchKind>,
        pairs: Vec<(Option<&str>, Option<&str>)>,
        fixed: bool,
        as_rung: bool,
    ) -> OrderRecord {
        OrderRecord {
            order_num: n,
            mode: "drive_thru".into(),
            ts: 1_792_800_000 + n as i64,
            outcome: if caught > 0 {
                Outcome::Mismatch
            } else {
                Outcome::Match
            },
            caught,
            kinds,
            pairs: pairs
                .into_iter()
                .map(|(a, b)| (a.map(Into::into), b.map(Into::into)))
                .collect(),
            fixed,
            sent_as_rung: as_rung,
            standards: Some(BTreeMap::from([("my_pleasure".into(), n % 2 == 1)])),
            latency_ms: 100 * n,
        }
    }

    #[test]
    fn schema_has_no_text_content() {
        let r = Records::open_in_memory().unwrap();
        assert_eq!(
            r.column_names().unwrap(),
            [
                "id",
                "order_num",
                "mode",
                "ts",
                "outcome",
                "caught",
                "kinds",
                "pairs",
                "fixed",
                "sent_as_rung",
                "standards",
                "latency_ms"
            ]
        );
    }

    #[test]
    fn dashboard_aggregates() {
        let r = Records::open_in_memory().unwrap();
        r.insert(&rec(1, 0, vec![], vec![], false, false)).unwrap();
        r.insert(&rec(
            2,
            1,
            vec![MismatchKind::Variant],
            vec![(Some("spicy_sandwich"), Some("chicken_sandwich"))],
            true,
            false,
        ))
        .unwrap();
        r.insert(&rec(
            3,
            1,
            vec![MismatchKind::Count],
            vec![(Some("polynesian"), Some("polynesian"))],
            false,
            true,
        ))
        .unwrap();
        let d = r.dashboard(&cfa(), 0).unwrap();
        assert_eq!(d.orders, 3);
        assert_eq!(d.orders_with_catch, 2);
        assert_eq!(d.caught_total, 2);
        assert!((d.fix_rate - 0.5).abs() < 1e-6);
        assert_eq!(d.by_kind["variant"], 1);
        assert_eq!(d.by_kind["count"], 1);
        assert_eq!(
            d.top_pairs[0],
            (
                "Spicy Chicken Sandwich".to_string(),
                "Chicken Sandwich".to_string(),
                1
            )
        );
        assert_eq!(d.by_hour.iter().map(|h| h.1).sum::<u32>(), 3);
        assert_eq!(d.by_hour.iter().map(|h| h.2).sum::<u32>(), 2);
        assert!((d.standards_rates["my_pleasure"] - 2.0 / 3.0).abs() < 1e-6);
        assert_eq!(d.p50_latency_ms, 200);
        assert!(!d.demo_data);
    }

    #[test]
    fn dashboard_respects_since() {
        let r = Records::open_in_memory().unwrap();
        r.insert(&rec(1, 0, vec![], vec![], false, false)).unwrap();
        assert_eq!(r.dashboard(&cfa(), 1_792_900_000).unwrap().orders, 0);
    }

    #[test]
    fn seed_demo_marks_mode() {
        let r = Records::open_in_memory().unwrap();
        seed_demo(&r, &cfa(), 300).unwrap();
        let d = r.dashboard(&cfa(), 0).unwrap();
        assert_eq!(d.orders, 300);
        assert!(d.demo_data);
        assert!(
            d.orders_with_catch > 0 && d.orders_with_catch < 150,
            "{}",
            d.orders_with_catch
        );
        assert!(!d.top_pairs.is_empty());
        let modes: Vec<String> = r
            .conn()
            .prepare("SELECT DISTINCT mode FROM orders")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(modes, ["demo"]);
    }

    #[test]
    fn records_survive_reopen() {
        let path = std::env::temp_dir().join(format!("oc-records-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        Records::open(&path)
            .unwrap()
            .insert(&rec(1, 0, vec![], vec![], false, false))
            .unwrap();
        assert_eq!(
            Records::open(&path)
                .unwrap()
                .dashboard(&cfa(), 0)
                .unwrap()
                .orders,
            1
        );
        std::fs::remove_file(&path).unwrap();
    }
}
