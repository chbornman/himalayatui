use std::time::Instant;

fn main() {
    // Check for --clear-cache flag
    let clear_cache = std::env::args().any(|a| a == "--clear-cache");

    if clear_cache {
        if let Some(cache_dir) = dirs::cache_dir() {
            let cache_file = cache_dir.join("mailtui/envelopes.bin");
            let _ = std::fs::remove_file(&cache_file);
            println!("Cleared cache");
        }
    }

    let mail_dir = shellexpand::tilde("~/Mail/gmail").to_string();
    let user_email = "calebbornman@gmail.com";

    println!("Scanning: {}/[Gmail]/All Mail", mail_dir);
    println!(
        "Available parallelism: {:?}",
        std::thread::available_parallelism()
    );

    let start = Instant::now();

    match mailtui::mail::scan_all_mail(&mail_dir, user_email, |current, total| {
        if current % 5000 == 0 {
            println!("Scan progress: {}/{}", current, total);
        }
    }) {
        Ok(envelopes) => {
            let scan_duration = start.elapsed();
            println!(
                "Scanned {} envelopes in {:?}",
                envelopes.len(),
                scan_duration
            );
            println!(
                "Rate: {:.0} emails/sec",
                envelopes.len() as f64 / scan_duration.as_secs_f64()
            );

            // Now benchmark threading
            println!("\nBuilding threads...");
            let thread_start = Instant::now();
            let threaded = mailtui::mail::build_threaded_list(envelopes);
            let thread_duration = thread_start.elapsed();

            println!(
                "Built {} threaded items in {:?}",
                threaded.len(),
                thread_duration
            );
            println!("\nTotal: {:?}", start.elapsed());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
