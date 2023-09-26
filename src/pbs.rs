use crate::Record;
pub use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use std::fmt::Write;

use std::collections::HashMap;

pub async fn init(m: &MultiProgress, total_mints: usize) -> HashMap<&'static str, ProgressBar> {
    let style = ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] ({pos}/{len} {msg}, ETA {eta})")
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .progress_chars("#>-");

    let mut progress_bars = HashMap::new();

    let pb = m.add(ProgressBar::new(total_mints as u64));
    pb.set_style(style.clone());
    pb.set_message("mint req");
    progress_bars.insert("mints", pb);

    let pb2 = m.insert_after(
        &progress_bars["mints"],
        ProgressBar::new(total_mints as u64),
    );
    pb2.set_style(style.clone());
    pb2.set_message("successful mints");
    progress_bars.insert("successful", pb2);

    let pb3 = m.insert_after(
        &progress_bars["successful"],
        ProgressBar::new(total_mints as u64),
    );
    pb3.set_style(style.clone());
    pb3.set_message("failed mints");
    progress_bars.insert("failed", pb3);

    let pb4 = m.insert_after(
        &progress_bars["failed"],
        ProgressBar::new(total_mints as u64),
    );
    pb4.set_style(style.clone());
    pb4.set_message("retries");
    progress_bars.insert("retries", pb4);

    progress_bars
}

pub async fn finalize(pb2: &ProgressBar, records: &[Record]) {
    let failed_mints = records.iter().filter(|&record| !record.success).count();

    if failed_mints > 0 {
        pb2.finish_with_message(format!("{} mints failed!", failed_mints));
    } else {
        pb2.finish_with_message("All mints created successfully!");
    }
}
