//! Console reporter — bucket-sorts findings by severity and writes the
//! shape from `rules/findings-format.md` verbatim.
//!
//! Report layout:
//!
//! ```text
//! ## Findings
//!
//! ### Must Fix
//! - `path/to/file.ext:LINE` — <what>. **Why:** <why>. **Fix:** <fix>.
//!
//! ### Should Fix
//! ...
//!
//! ### Consider
//! ...
//!
//! N finding(s) — X Must Fix, Y Should Fix, Z Consider.
//! ```
//!
//! Empty buckets are omitted (per the findings-format rule — never
//! print an empty `### Must Fix` header). A run with zero findings
//! prints a single-line success message instead of the full header.

use std::io::Write;

use crate::domain::{Finding, Severity};

pub fn write_console<W: Write>(w: &mut W, findings: &[Finding]) -> std::io::Result<()> {
    if findings.is_empty() {
        writeln!(w, "plug-audit: no findings.")?;
        return Ok(());
    }

    let mut must = Vec::new();
    let mut should = Vec::new();
    let mut consider = Vec::new();
    for f in findings {
        match f.severity {
            Severity::MustFix => must.push(f),
            Severity::ShouldFix => should.push(f),
            Severity::Consider => consider.push(f),
        }
    }

    writeln!(w, "## Findings")?;
    writeln!(w)?;

    write_bucket(w, Severity::MustFix, &must)?;
    write_bucket(w, Severity::ShouldFix, &should)?;
    write_bucket(w, Severity::Consider, &consider)?;

    writeln!(
        w,
        "{} finding(s) — {} Must Fix, {} Should Fix, {} Consider.",
        findings.len(),
        must.len(),
        should.len(),
        consider.len()
    )?;
    Ok(())
}

fn write_bucket<W: Write>(
    w: &mut W,
    severity: Severity,
    items: &[&Finding],
) -> std::io::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    writeln!(w, "### {}", severity.heading())?;
    writeln!(w)?;
    for f in items {
        // Append the rule ID as a bracketed suffix so users can grep
        // for it when adding a suppression. Display's shape (locked by
        // the PA-2 snapshots) stays intact — the CLI decorates.
        writeln!(w, "- {f}  [{}]", f.rule.as_str())?;
    }
    writeln!(w)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::finding::{ByteSpan, Location};
    use crate::domain::rule::RuleId;
    use std::path::PathBuf;

    fn finding(rule_id: &str, severity: Severity, line: u32, msg: &str) -> Finding {
        Finding {
            rule: RuleId::parse(rule_id).unwrap(),
            severity,
            location: Location {
                file: PathBuf::from("plugin/x.lua"),
                line,
                column: 1,
                byte_span: ByteSpan::new(0, 0),
            },
            message: msg.to_string(),
            why: "reason".to_string(),
            fix: Some("fix".to_string()),
        }
    }

    fn render(findings: &[Finding]) -> String {
        let mut out = Vec::new();
        write_console(&mut out, findings).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn empty_findings_produce_success_line() {
        let out = render(&[]);
        assert_eq!(out.trim(), "plug-audit: no findings.");
    }

    #[test]
    fn buckets_render_in_severity_order() {
        let findings = vec![
            finding("nvim/health-check", Severity::MustFix, 1, "must one"),
            finding("nvim/plug-mapping", Severity::ShouldFix, 5, "should one"),
            finding("nvim/plug-mapping", Severity::ShouldFix, 6, "should two"),
        ];
        let out = render(&findings);
        insta::assert_snapshot!("console_report_shape", out);
    }

    #[test]
    fn empty_buckets_are_omitted() {
        let findings = vec![finding("nvim/plug-mapping", Severity::ShouldFix, 5, "solo")];
        let out = render(&findings);
        assert!(!out.contains("### Must Fix"));
        assert!(!out.contains("### Consider"));
        assert!(out.contains("### Should Fix"));
    }

    #[test]
    fn summary_counts_are_correct() {
        let findings = vec![
            finding("nvim/health-check", Severity::MustFix, 1, "m"),
            finding("nvim/plug-mapping", Severity::ShouldFix, 5, "s"),
            finding("nvim/plug-mapping", Severity::ShouldFix, 6, "s"),
        ];
        let out = render(&findings);
        assert!(out.contains("3 finding(s) — 1 Must Fix, 2 Should Fix, 0 Consider."));
    }
}
