fn main() {
    let mail_dir = shellexpand::tilde("~/Mail/gmail").to_string();
    let user_email = "calebbornman@gmail.com";

    let envelopes = mailtui::mail::scan_all_mail(&mail_dir, user_email, |_, _| {}).unwrap();

    println!("Sample dates from envelopes:");
    for env in envelopes.iter().take(20) {
        let subj = env.subject.as_deref().unwrap_or("(none)");
        let date = env.date.as_deref().unwrap_or("NO DATE");
        println!("  {} - {}", date, &subj[..subj.len().min(40)]);
    }

    // Find min and max dates
    let dates: Vec<_> = envelopes.iter().filter_map(|e| e.date.as_ref()).collect();
    let min = dates.iter().min();
    let max = dates.iter().max();
    println!("\nMin date: {:?}", min);
    println!("Max date: {:?}", max);

    // Check if dates are ISO format
    println!("\nDate format check (first 5 with dates):");
    for env in envelopes.iter().filter(|e| e.date.is_some()).take(5) {
        println!("  {:?}", env.date);
    }
}
