//! Read-only smoke check. Prints totals, never filenames or file contents.
use disk_cleaner_tauri::inventory::{self, model::Protection, rules};
use std::{collections::BTreeMap, sync::atomic::AtomicBool, time::Instant};
fn main() -> Result<(), String> {
    let started = Instant::now();
    let roots = rules::Roots::current()?;
    let mut rules = rules::default_rules(&roots);
    rules.extend(rules::large_file_rules(&roots));
    let (result, _) = inventory::scan(rules, &AtomicBool::new(false), |progress| {
        eprintln!(
            "scan {}/{} elapsed={}ms",
            progress.completed,
            progress.total,
            started.elapsed().as_millis()
        );
    });
    let mut categories = BTreeMap::<String, usize>::new();
    for item in &result.items {
        *categories.entry(item.category.clone()).or_default() += 1;
    }
    println!(
        "{}",
        serde_json::json!({
            "items": result.items.len(), "categories": categories,
            "safe": result.items.iter().filter(|i| i.protection == Protection::Safe).count(),
            "review": result.items.iter().filter(|i| i.protection == Protection::Review).count(),
            "blocked": result.items.iter().filter(|i| i.protection == Protection::Blocked).count(),
            "warnings": result.warnings.len(), "drives": result.drives.len(),
            "elapsedMs": started.elapsed().as_millis(), "cancelled": result.cancelled
        })
    );
    Ok(())
}
