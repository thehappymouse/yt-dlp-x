const PROGRESS_PREFIX: &str = "download:__YTDLPX__:";

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressInfo {
    pub percent: f64,
    pub percent_str: String,
    pub eta: Option<String>,
    pub speed: Option<String>,
    pub total: Option<String>,
    pub status: Option<String>,
    pub raw: String,
}

pub fn progress_template_value() -> &'static str {
    "download:__YTDLPX__:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(progress._total_bytes_str)s"
}

pub fn parse_progress_line(line: &str) -> Option<ProgressInfo> {
    parse_template_progress_line(line).or_else(|| parse_legacy_download_line(line))
}

fn parse_template_progress_line(line: &str) -> Option<ProgressInfo> {
    if !line.starts_with(PROGRESS_PREFIX) {
        return None;
    }

    let raw = line.trim();
    let payload = raw.trim_start_matches(PROGRESS_PREFIX);
    let mut parts = payload.split('|');

    let percent_part = parts.next()?.trim();
    let speed_part = parts.next().unwrap_or_default().trim();
    let eta_part = parts.next().unwrap_or_default().trim();
    let total_part = parts.next().unwrap_or_default().trim();

    let percent = parse_percent(percent_part)?;
    let percent_str = format!("{}%", trim_percent_symbol(percent_part));
    let speed = normalize_token(speed_part);
    let eta = normalize_token(eta_part);
    let total = normalize_token(total_part);

    let status = if percent >= 100.0 {
        Some("finished".to_string())
    } else {
        Some("downloading".to_string())
    };

    Some(ProgressInfo {
        percent,
        percent_str,
        eta,
        speed,
        total,
        status,
        raw: payload.trim().to_string(),
    })
}

fn parse_legacy_download_line(line: &str) -> Option<ProgressInfo> {
    if !line.starts_with("[download]") {
        return None;
    }

    let trimmed = line.trim_start_matches("[download]").trim();
    let (percent_part, rest_part) = trimmed.split_once('%')?;
    let percent_str = percent_part.trim();
    if percent_str.is_empty() {
        return None;
    }

    let percent_value = percent_str.parse::<f64>().ok()?;
    let percent_display = format!("{percent_str}%");
    let rest = rest_part.trim();

    let mut eta = None;

    if let Some(index) = rest.rfind("ETA ") {
        let value = rest[index + 4..].trim();
        if !value.is_empty() {
            eta = Some(value.to_string());
        }
    } else if let Some(index) = rest.rfind(" in ") {
        let value = rest[index + 4..].trim();
        if !value.is_empty() {
            eta = Some(value.to_string());
        }
    }

    let mut speed = None;
    if let Some(index) = rest.find(" at ") {
        let after = &rest[index + 4..];
        if let Some(token) = after.split_whitespace().next() {
            let cleaned = token.trim().trim_end_matches(',');
            if !cleaned.is_empty() {
                speed = Some(cleaned.to_string());
            }
        }
    }

    let mut total = None;
    if let Some(after_of) = rest.trim_start().strip_prefix("of ") {
        let mut end_index = after_of.len();
        for marker in [" at ", " ETA ", " in "] {
            if let Some(idx) = after_of.find(marker) {
                end_index = end_index.min(idx);
            }
        }
        let candidate = after_of[..end_index].trim().trim_end_matches(',');
        if !candidate.is_empty() {
            total = Some(candidate.to_string());
        }
    }

    let mut status = None;
    if rest.contains(" in ") || percent_value >= 100.0 {
        status = Some("finished".to_string());
    } else if rest.contains("ETA") || !rest.is_empty() {
        status = Some("downloading".to_string());
    }

    Some(ProgressInfo {
        percent: percent_value,
        percent_str: percent_display,
        eta,
        speed,
        total,
        status,
        raw: trimmed.to_string(),
    })
}

fn parse_percent(value: &str) -> Option<f64> {
    let normalized = trim_percent_symbol(value);
    if normalized.is_empty() {
        return None;
    }
    normalized.parse::<f64>().ok()
}

fn trim_percent_symbol(value: &str) -> String {
    value
        .trim()
        .trim_end_matches('%')
        .trim()
        .to_string()
}

fn normalize_token(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("n/a")
        || trimmed.eq_ignore_ascii_case("na")
        || trimmed == "--"
    {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_progress_line, progress_template_value};

    #[test]
    fn parses_template_progress_line() {
        let line = "download:__YTDLPX__: 54.3%|1.20MiB/s|00:13|33.90MiB";
        let parsed = parse_progress_line(line).expect("line should parse");

        assert_eq!(parsed.percent, 54.3);
        assert_eq!(parsed.percent_str, "54.3%");
        assert_eq!(parsed.speed.as_deref(), Some("1.20MiB/s"));
        assert_eq!(parsed.eta.as_deref(), Some("00:13"));
        assert_eq!(parsed.total.as_deref(), Some("33.90MiB"));
        assert_eq!(parsed.status.as_deref(), Some("downloading"));
    }

    #[test]
    fn parses_legacy_download_line_as_fallback() {
        let line = "[download] 100% of 12.12MiB at 1.00MiB/s in 00:12";
        let parsed = parse_progress_line(line).expect("line should parse");

        assert_eq!(parsed.percent, 100.0);
        assert_eq!(parsed.percent_str, "100%");
        assert_eq!(parsed.status.as_deref(), Some("finished"));
    }

    #[test]
    fn progress_template_contains_expected_prefix() {
        let template = progress_template_value();
        assert!(template.starts_with("download:__YTDLPX__:"));
    }
}
