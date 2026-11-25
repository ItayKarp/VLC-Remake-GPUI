//! "Report on GitHub" (S9.6.1): the opt-in way a crash report leaves the computer. VLC sends
//! nothing itself; it opens the project's new-issue page in the browser with the saved report
//! filled in, and the user reads it there and decides whether to submit it. The report is the
//! file as saved, so it holds file names only if the user allowed them (S9.6.2).

use url::Url;

/// The page that opens a new issue on the project.
const NEW_ISSUE_URL: &str = "https://github.com/ItayKarp/VLC-Remake-GPUI/issues/new";
/// The label crash issues get, where the reporter may set labels.
const LABEL: &str = "crash";
/// The longest address opened. GitHub refuses addresses past about 8 KB; this leaves room.
const MAX_URL_BYTES: usize = 7_500;
/// The longest title, in characters, before it is cut.
const MAX_TITLE_CHARS: usize = 80;
/// The report line holding the panic's message.
const MESSAGE_PREFIX: &str = "Message: ";

/// The new-issue address for `report`. When the whole report doesn't fit in an address, its
/// last lines (the deepest frames of the backtrace) are left out, and the issue says so.
pub fn new_issue_url(report: &str) -> Url {
    let lines: Vec<&str> = report.lines().collect();
    let title = title(report);
    let url_keeping = |kept: usize| issue_url(&title, &body(&lines[..kept], kept < lines.len()));
    let fits = |kept: usize| url_keeping(kept).as_str().len() <= MAX_URL_BYTES;
    if fits(lines.len()) {
        return url_keeping(lines.len());
    }
    // The most lines that fit: `fits(low)` holds (or `low` is 0) and `fits(high)` doesn't.
    let (mut low, mut high) = (0, lines.len());
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if fits(middle) { low = middle } else { high = middle }
    }
    url_keeping(low)
}

/// "Crash: " and the first line of the panic's message.
fn title(report: &str) -> String {
    let message = report
        .lines()
        .find_map(|line| line.strip_prefix(MESSAGE_PREFIX))
        .map_or("VLC closed unexpectedly", str::trim);
    let short: String = message.chars().take(MAX_TITLE_CHARS).collect();
    let cut = if short.len() < message.len() { "…" } else { "" };
    format!("Crash: {short}{cut}")
}

/// The issue's text: a question for the reporter, then the report in a fold.
fn body(lines: &[&str], shortened: bool) -> String {
    let note = if shortened {
        "\n(Shortened to fit. The full report is in the crash report file VLC saved.)\n"
    } else {
        ""
    };
    format!(
        "**What were you doing when VLC closed?**\n\n\n\n\
         <details><summary>Crash report</summary>\n\n```\n{}\n```\n{note}</details>\n",
        lines.join("\n"),
    )
}

fn issue_url(title: &str, body: &str) -> Url {
    let pairs = [("title", title), ("body", body), ("labels", LABEL)];
    Url::parse_with_params(NEW_ISSUE_URL, pairs).expect("the new-issue address is a valid URL")
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    fn query(url: &Url, key: &str) -> String {
        url.query_pairs()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.into_owned())
            .unwrap()
    }

    #[test]
    fn the_issue_is_titled_after_the_message_and_holds_the_report() {
        let report = "VLC crash report\nVersion: 1.0\nMessage: index out of bounds\n\nBacktrace:\n";
        let url = new_issue_url(report);
        assert!(url.as_str().starts_with(NEW_ISSUE_URL), "{url}");
        assert_eq!(query(&url, "title"), "Crash: index out of bounds");
        assert!(query(&url, "body").contains(report.trim_end()), "{url}");
        assert!(!query(&url, "body").contains("Shortened"), "{url}");
        assert_eq!(query(&url, "labels"), LABEL);
    }

    #[test]
    fn a_long_report_is_cut_to_fit_from_the_end() {
        let mut report = String::from("VLC crash report\nMessage: boom\n\nBacktrace:\n");
        for n in 0..2_000 {
            writeln!(report, "  {n}: frame::number_{n}").unwrap();
        }
        let url = new_issue_url(&report);
        assert!(url.as_str().len() <= MAX_URL_BYTES, "{}", url.as_str().len());
        let body = query(&url, "body");
        assert!(body.contains("Message: boom"), "{body}");
        assert!(body.contains("Shortened to fit"), "{body}");
        assert!(!body.contains("number_1999"), "{body}");
    }

    #[test]
    fn a_report_without_a_message_gets_a_plain_title() {
        assert_eq!(title("VLC crash report\n"), "Crash: VLC closed unexpectedly");
        let long = format!("Message: {}", "x".repeat(200));
        assert_eq!(title(&long).chars().count(), "Crash: ".len() + MAX_TITLE_CHARS + 1);
    }
}
