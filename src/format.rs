use crate::diff::Change;

fn color_for(change: &Change) -> String {
    match change {
        Change::New(_) => cybercore::palette::cyan(),
        Change::Deleted(_) => cybercore::palette::red(),
        Change::Modified { .. } => cybercore::palette::orange(),
    }
}

fn reset() -> &'static str {
    cybercore::palette::RESET
}

pub fn render_report(changes: &[Change], color_on: bool) -> String {
    if changes.is_empty() {
        return "No drift detected — everything matches the baseline.\n".to_string();
    }

    let mut out = String::new();
    for change in changes {
        let (c, r) = if color_on { (color_for(change), reset().to_string()) } else { (String::new(), String::new()) };
        match change {
            Change::New(path) => out.push_str(&format!("{c}[NEW]     {path}{r}\n")),
            Change::Deleted(path) => out.push_str(&format!("{c}[DELETED] {path}{r}\n")),
            Change::Modified { path, content_changed, mode_changed, owner_changed } => {
                let mut reasons = Vec::new();
                if *content_changed {
                    reasons.push("content");
                }
                if *mode_changed {
                    reasons.push("permissions");
                }
                if *owner_changed {
                    reasons.push("ownership");
                }
                out.push_str(&format!("{c}[MODIFIED]{r} {path} ({})\n", reasons.join(", ")));
            }
        }
    }
    out.push_str(&format!("\n{} change(s) from baseline.\n", changes.len()));
    out
}
