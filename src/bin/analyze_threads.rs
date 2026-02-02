fn main() {
    let mail_dir = shellexpand::tilde("~/Mail/gmail").to_string();
    let user_email = "calebbornman@gmail.com";

    println!("Scanning...");
    let envelopes = mailtui::mail::scan_all_mail(&mail_dir, user_email, |_, _| {}).unwrap();
    println!("Total envelopes: {}", envelopes.len());

    let threaded = mailtui::mail::build_threaded_list(envelopes.clone());

    // Count threads by looking at depth=0 messages
    let num_threads = threaded.iter().filter(|e| e.thread_depth == 0).count();

    println!("\nThreaded results:");
    println!("  Total messages: {}", threaded.len());
    println!("  Threads: {}", num_threads);

    // Thread size distribution - count by thread roots
    let mut thread_sizes: Vec<(usize, Option<String>)> = Vec::new();
    let mut current_size = 0;
    let mut current_subject: Option<String> = None;
    for env in &threaded {
        if env.thread_depth == 0 {
            // Start of new thread
            if current_size > 0 {
                thread_sizes.push((current_size, current_subject.clone()));
            }
            current_size = 1;
            current_subject = env.subject.clone();
        } else {
            current_size += 1;
        }
    }
    if current_size > 0 {
        thread_sizes.push((current_size, current_subject));
    }

    thread_sizes.sort_by(|a, b| b.0.cmp(&a.0));

    println!("\nThread size distribution:");
    println!(
        "  Single message threads: {}",
        thread_sizes.iter().filter(|(s, _)| *s == 1).count()
    );
    println!(
        "  2-5 messages: {}",
        thread_sizes
            .iter()
            .filter(|(s, _)| *s >= 2 && *s <= 5)
            .count()
    );
    println!(
        "  6-10 messages: {}",
        thread_sizes
            .iter()
            .filter(|(s, _)| *s >= 6 && *s <= 10)
            .count()
    );
    println!(
        "  11-50 messages: {}",
        thread_sizes
            .iter()
            .filter(|(s, _)| *s >= 11 && *s <= 50)
            .count()
    );
    println!(
        "  50+ messages: {}",
        thread_sizes.iter().filter(|(s, _)| *s > 50).count()
    );

    println!("\nTop 20 largest threads:");
    for (i, (size, subj)) in thread_sizes.iter().take(20).enumerate() {
        let subject = subj.as_deref().unwrap_or("(no subject)");
        let truncated: String = subject.chars().take(50).collect();
        println!("  {:2}. {:4} msgs - {}", i + 1, size, truncated);
    }

    // Check for duplicate message-ids
    println!("\n--- Checking for issues ---");

    let mut message_ids: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for env in &envelopes {
        if let Some(ref mid) = env.message_id {
            *message_ids.entry(mid.clone()).or_default() += 1;
        }
    }
    let duplicates: Vec<_> = message_ids
        .iter()
        .filter(|&(_, count)| *count > 1)
        .collect();
    println!("Duplicate message-ids: {}", duplicates.len());
    for (mid, count) in duplicates.iter().take(5) {
        println!("  {} appears {} times", &mid[..mid.len().min(60)], count);
    }

    // Count messages without message-id
    let no_mid = envelopes.iter().filter(|e| e.message_id.is_none()).count();
    println!("Messages without message-id: {}", no_mid);

    // Count messages with in_reply_to but no matching parent
    let id_set: std::collections::HashSet<_> = envelopes
        .iter()
        .filter_map(|e| e.message_id.as_ref())
        .collect();
    let orphan_replies = envelopes
        .iter()
        .filter(|e| {
            if let Some(ref reply_to) = e.in_reply_to {
                !id_set.contains(reply_to)
            } else {
                false
            }
        })
        .count();
    println!("Orphan replies (in_reply_to not found): {}", orphan_replies);
}
