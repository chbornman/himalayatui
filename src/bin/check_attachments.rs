fn main() {
    // Email with attachment (calendar invite)
    let test_file1 = "/home/caleb/Mail/gmail/Inbox/cur/1770006585.252281_22.asahi,U=22:2,S";
    // Email with images
    let test_file2 = "/home/caleb/Mail/gmail/Inbox/cur/1770006583.252281_1.asahi,U=1:2,RS";

    for (label, path) in [("Attachment test", test_file1), ("Image test", test_file2)] {
        println!("\n=== {} ===", label);
        println!("File: {}\n", path);

        match mailtui::mail::read_message_by_path(path) {
            Ok(text) => {
                // Just show the footer part (last 20 lines or from separator)
                let lines: Vec<&str> = text.lines().collect();
                let sep_idx = lines
                    .iter()
                    .rposition(|l| l.contains("───────────"))
                    .unwrap_or(lines.len().saturating_sub(20));
                for line in &lines[sep_idx..] {
                    println!("{}", line);
                }
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}
