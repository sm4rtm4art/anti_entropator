//! Output formatting for profile results

use crate::domain::stats::ProfileResult;
use anyhow::Result;
use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, ContentArrangement, Table,
};
use humansize::{format_size, BINARY, DECIMAL};

/// Format bytes as human-readable string
pub fn format_bytes(bytes: u64, decimal: bool) -> String {
    if decimal {
        format_size(bytes, DECIMAL)
    } else {
        format_size(bytes, BINARY)
    }
}

/// Print profile results as formatted tables
pub fn print_table_report(result: &ProfileResult, decimal: bool) -> Result<()> {
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  📊 Anti-Entropator Swamp Profile");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  Path: {}", result.path);
    println!(
        "  Files: {} | Dirs: {} | Symlinks: {}",
        result.file_count, result.dir_count, result.symlink_count
    );
    println!(
        "  Total size: {} | Zero-byte files: {}",
        format_bytes(result.total_bytes, decimal),
        result.zero_byte_count
    );
    if !result.errors.is_empty() {
        println!("  Errors: {}", result.errors.len());
    }
    println!();

    // By extension table
    print_extension_table(result, decimal)?;

    // By category table
    print_category_table(result, decimal)?;

    // MIME type table (if available)
    if !result.by_mime.is_empty() {
        print_mime_table(result, decimal)?;
    }

    // Name patterns
    print_name_patterns(result)?;

    // Duplicate estimate
    print_duplicate_estimate(result, decimal)?;

    // Largest files
    print_largest_files(result, decimal)?;

    // No-extension examples
    if !result.no_extension_examples.is_empty() {
        print_no_extension_examples(result)?;
    }

    // Errors sample
    if !result.errors.is_empty() {
        print_errors_sample(result)?;
    }

    Ok(())
}

fn print_extension_table(result: &ProfileResult, decimal: bool) -> Result<()> {
    println!("─── By Extension (top 25 by total size) ───────────────────────");
    println!();

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Extension").fg(Color::Cyan),
            Cell::new("Count").fg(Color::Cyan),
            Cell::new("Total").fg(Color::Cyan),
            Cell::new("Avg").fg(Color::Cyan),
            Cell::new("Min").fg(Color::Cyan),
            Cell::new("Max").fg(Color::Cyan),
            Cell::new("P50").fg(Color::Cyan),
            Cell::new("P90").fg(Color::Cyan),
        ]);

    // Sort by total bytes
    let mut entries: Vec<_> = result.by_extension.iter().collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.1.total_bytes));

    for (ext, stats) in entries.iter().take(25) {
        table.add_row(vec![
            Cell::new(ext),
            Cell::new(stats.count.to_string()),
            Cell::new(format_bytes(stats.total_bytes, decimal)),
            Cell::new(format_bytes(stats.avg_bytes() as u64, decimal)),
            Cell::new(format_bytes(stats.min_bytes, decimal)),
            Cell::new(format_bytes(stats.max_bytes, decimal)),
            Cell::new(format_bytes(stats.p50(), decimal)),
            Cell::new(format_bytes(stats.p90(), decimal)),
        ]);
    }

    println!("{table}");
    println!();

    Ok(())
}

fn print_category_table(result: &ProfileResult, decimal: bool) -> Result<()> {
    println!("─── By Category ────────────────────────────────────────────────");
    println!();

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Category").fg(Color::Cyan),
            Cell::new("Count").fg(Color::Cyan),
            Cell::new("Total").fg(Color::Cyan),
            Cell::new("% of Total").fg(Color::Cyan),
        ]);

    let mut entries: Vec<_> = result.by_category.iter().collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.1.total_bytes));

    for (cat, stats) in entries {
        let pct = if result.total_bytes > 0 {
            (stats.total_bytes as f64 / result.total_bytes as f64) * 100.0
        } else {
            0.0
        };

        table.add_row(vec![
            Cell::new(cat),
            Cell::new(stats.count.to_string()),
            Cell::new(format_bytes(stats.total_bytes, decimal)),
            Cell::new(format!("{:.1}%", pct)),
        ]);
    }

    println!("{table}");
    println!();

    Ok(())
}

fn print_mime_table(result: &ProfileResult, decimal: bool) -> Result<()> {
    println!("─── By MIME Type (top 15) ──────────────────────────────────────");
    println!();

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("MIME Type").fg(Color::Cyan),
            Cell::new("Count").fg(Color::Cyan),
            Cell::new("Total").fg(Color::Cyan),
        ]);

    let mut entries: Vec<_> = result.by_mime.iter().collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.1.total_bytes));

    for (mime, stats) in entries.iter().take(15) {
        table.add_row(vec![
            Cell::new(mime),
            Cell::new(stats.count.to_string()),
            Cell::new(format_bytes(stats.total_bytes, decimal)),
        ]);
    }

    println!("{table}");
    println!();

    Ok(())
}

fn print_name_patterns(result: &ProfileResult) -> Result<()> {
    if result.name_patterns.is_empty() {
        return Ok(());
    }

    println!("─── Name Quality Patterns ──────────────────────────────────────");
    println!();

    let mut entries: Vec<_> = result.name_patterns.iter().collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(*entry.1));

    for (pattern, count) in entries {
        let pct = if result.file_count > 0 {
            (*count as f64 / result.file_count as f64) * 100.0
        } else {
            0.0
        };
        println!("  {:<20} {:>6} ({:>5.1}%)", pattern, count, pct);
    }
    println!();

    Ok(())
}

fn print_duplicate_estimate(result: &ProfileResult, decimal: bool) -> Result<()> {
    let est = &result.duplicate_estimate;

    println!("─── Duplicate Estimate (size + quick-hash) ─────────────────────");
    println!();
    println!("  Size-candidate groups: {}", est.size_candidate_groups);
    println!(
        "  Quick-hash confirmed groups: {}",
        est.quickhash_confirmed_groups
    );
    println!("  Files hashed: {}", est.files_hashed);
    println!(
        "  Estimated reclaimable: {}",
        format_bytes(est.reclaimable_bytes, decimal)
    );
    println!();

    if !est.top_groups.is_empty() {
        println!("  Top duplicate groups:");
        for group in &est.top_groups {
            println!(
                "    • {} files × {} each",
                group.count,
                format_bytes(group.size_bytes, decimal)
            );
            for path in group.sample_paths.iter().take(3) {
                println!("      - {}", path);
            }
            if group.sample_paths.len() > 3 {
                println!("      - ... +{} more", group.sample_paths.len() - 3);
            }
        }
        println!();
    }

    Ok(())
}

fn print_largest_files(result: &ProfileResult, decimal: bool) -> Result<()> {
    println!("─── Largest Files (top 15) ─────────────────────────────────────");
    println!();

    for (size, path) in &result.largest_files {
        println!("  {:>12}  {}", format_bytes(*size, decimal), path);
    }
    println!();

    Ok(())
}

fn print_no_extension_examples(result: &ProfileResult) -> Result<()> {
    println!("─── Files Without Extension (sample) ───────────────────────────");
    println!();

    for path in result.no_extension_examples.iter().take(10) {
        println!("  - {}", path);
    }
    if result.no_extension_examples.len() > 10 {
        println!("  ... +{} more", result.no_extension_examples.len() - 10);
    }
    println!();

    Ok(())
}

fn print_errors_sample(result: &ProfileResult) -> Result<()> {
    println!("─── Errors (sample) ────────────────────────────────────────────");
    println!();

    for err in result.errors.iter().take(10) {
        println!("  - {}: {}", err.path, err.error);
    }
    if result.errors.len() > 10 {
        println!("  ... +{} more", result.errors.len() - 10);
    }
    println!();

    Ok(())
}

/// Print profile result as JSON
pub fn print_json_report(result: &ProfileResult) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(result)?);
    Ok(())
}

/// Print profile result as Markdown
pub fn print_markdown_report(result: &ProfileResult, decimal: bool) -> Result<()> {
    let md = generate_markdown_report(result, decimal)?;
    println!("{}", md);
    Ok(())
}

/// Generate Markdown report string
pub fn generate_markdown_report(result: &ProfileResult, decimal: bool) -> Result<String> {
    let mut md = String::new();

    md.push_str("# Anti-Entropator Swamp Profile\n\n");
    md.push_str(&format!("**Path:** `{}`\n\n", result.path));
    md.push_str(&format!(
        "| Metric | Value |\n|--------|-------|\n| Files | {} |\n| Directories | {} |\n| Symlinks | {} |\n| Total Size | {} |\n| Zero-byte Files | {} |\n| Errors | {} |\n\n",
        result.file_count,
        result.dir_count,
        result.symlink_count,
        format_bytes(result.total_bytes, decimal),
        result.zero_byte_count,
        result.errors.len()
    ));

    // By extension
    md.push_str("## By Extension (top 25)\n\n");
    md.push_str("| Extension | Count | Total | Avg | Max |\n");
    md.push_str("|-----------|-------|-------|-----|-----|\n");

    let mut entries: Vec<_> = result.by_extension.iter().collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.1.total_bytes));

    for (ext, stats) in entries.iter().take(25) {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            ext,
            stats.count,
            format_bytes(stats.total_bytes, decimal),
            format_bytes(stats.avg_bytes() as u64, decimal),
            format_bytes(stats.max_bytes, decimal),
        ));
    }
    md.push('\n');

    // Duplicate estimate
    let est = &result.duplicate_estimate;
    md.push_str("## Duplicate Estimate\n\n");
    md.push_str(&format!(
        "- **Size-candidate groups:** {}\n- **Confirmed groups:** {}\n- **Estimated reclaimable:** {}\n\n",
        est.size_candidate_groups,
        est.quickhash_confirmed_groups,
        format_bytes(est.reclaimable_bytes, decimal)
    ));

    // Largest files
    md.push_str("## Largest Files\n\n");
    for (size, path) in result.largest_files.iter().take(10) {
        md.push_str(&format!(
            "- `{}` ({})\n",
            path,
            format_bytes(*size, decimal)
        ));
    }
    md.push('\n');

    Ok(md)
}
