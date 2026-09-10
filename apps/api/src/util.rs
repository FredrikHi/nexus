//! Small helpers shared by more than one feature module.

/// Turn a human name into a URL-safe slug: "Invoice Service" -> "invoice-service".
///
/// Runs of non-alphanumeric characters collapse into a single dash, and leading
/// or trailing dashes are trimmed. Returns an empty string when the input holds
/// no ASCII alphanumerics at all; callers treat that as a validation failure.
pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in input.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
            pending_dash = false;
        } else if !out.is_empty() && !pending_dash {
            out.push('-');
            pending_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn lowercases_and_dashes_words() {
        assert_eq!(slugify("Invoice Service"), "invoice-service");
    }

    #[test]
    fn collapses_runs_of_separators() {
        assert_eq!(slugify("Order   //  Sync"), "order-sync");
    }

    #[test]
    fn trims_leading_and_trailing_noise() {
        assert_eq!(slugify("  --Billing API--  "), "billing-api");
    }

    #[test]
    fn keeps_digits() {
        assert_eq!(slugify("Peppol BIS 3.0"), "peppol-bis-3-0");
    }

    #[test]
    fn yields_empty_when_nothing_survives() {
        assert_eq!(slugify("///"), "");
        assert_eq!(slugify(""), "");
    }
}
