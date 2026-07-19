//! JSON reporter — emits a stable machine-readable wire shape.
//!
//! Wire format (locked contract, snapshot-tested):
//!
//! ```json
//! {
//!   "version": "<crate version>",
//!   "findings": [ /* Finding — same shape as the domain snapshot */ ],
//!   "summary": {
//!     "total": 8,
//!     "must_fix": 1,
//!     "should_fix": 7,
//!     "consider": 0
//!   }
//! }
//! ```
//!
//! Design notes:
//! - The top level is an object, not a bare `findings` array. This
//!   lets us add non-breaking metadata later (config used, timings)
//!   and lets consumers distinguish "tool ran, zero findings" from
//!   "tool errored" by checking that the JSON parses and has a
//!   `findings` array.
//! - `version` is `env!("CARGO_PKG_VERSION")`, not a wire-format
//!   version. Consumers who need to feature-detect should look at the
//!   presence of specific fields, not compare version strings.
//! - The output ends with a trailing newline for POSIX friendliness
//!   (tools that append to logs, pipes to `less`, etc.).

use std::io::Write;

use serde::Serialize;

use crate::domain::{Finding, Severity};

/// Envelope emitted at the top level of every JSON report.
#[derive(Serialize)]
struct Report<'a> {
    version: &'static str,
    findings: &'a [Finding],
    summary: Summary,
}

#[derive(Serialize)]
struct Summary {
    total: usize,
    must_fix: usize,
    should_fix: usize,
    consider: usize,
}

pub fn write_json<W: Write>(w: &mut W, findings: &[Finding]) -> std::io::Result<()> {
    let summary = summarize(findings);
    let report = Report {
        version: env!("CARGO_PKG_VERSION"),
        findings,
        summary,
    };
    serde_json::to_writer_pretty(&mut *w, &report).map_err(std::io::Error::other)?;
    writeln!(w)
}

fn summarize(findings: &[Finding]) -> Summary {
    let mut s = Summary {
        total: findings.len(),
        must_fix: 0,
        should_fix: 0,
        consider: 0,
    };
    for f in findings {
        match f.severity {
            Severity::MustFix => s.must_fix += 1,
            Severity::ShouldFix => s.should_fix += 1,
            Severity::Consider => s.consider += 1,
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::finding::{ByteSpan, Location};
    use crate::domain::rule::RuleId;
    use std::path::PathBuf;

    fn finding(rule_id: &str, severity: Severity, line: u32) -> Finding {
        Finding {
            rule: RuleId::parse(rule_id).unwrap(),
            severity,
            location: Location {
                file: PathBuf::from("plugin/x.lua"),
                line,
                column: 1,
                byte_span: ByteSpan::new(10, 20),
            },
            message: "message".to_string(),
            why: "why".to_string(),
            fix: match severity {
                Severity::Consider => None,
                _ => Some("fix".to_string()),
            },
        }
    }

    fn render(findings: &[Finding]) -> String {
        let mut out = Vec::new();
        write_json(&mut out, findings).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn parse(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("output must be valid JSON")
    }

    #[test]
    fn empty_findings_produce_valid_json() {
        let out = render(&[]);
        let v = parse(&out);
        assert_eq!(v["findings"].as_array().unwrap().len(), 0);
        assert_eq!(v["summary"]["total"], 0);
        assert_eq!(v["summary"]["must_fix"], 0);
        assert_eq!(v["summary"]["should_fix"], 0);
        assert_eq!(v["summary"]["consider"], 0);
        assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn wire_shape_is_stable() {
        // Snapshot version-independent by cloning the JSON and
        // stripping the version field before comparison — the crate
        // version changes on every release and would otherwise churn
        // the snapshot.
        let findings = vec![
            finding("nvim/health-check", Severity::MustFix, 1),
            finding("nvim/plug-mapping", Severity::ShouldFix, 5),
            finding("nvim/plug-mapping", Severity::Consider, 10),
        ];
        let out = render(&findings);
        let mut v: serde_json::Value = parse(&out);
        v["version"] = serde_json::Value::String("REDACTED".to_string());
        insta::assert_json_snapshot!("json_report_shape", v);
    }

    #[test]
    fn summary_matches_severity_counts() {
        let findings = vec![
            finding("nvim/health-check", Severity::MustFix, 1),
            finding("nvim/health-check", Severity::MustFix, 2),
            finding("nvim/plug-mapping", Severity::ShouldFix, 5),
            finding("nvim/plug-mapping", Severity::Consider, 10),
        ];
        let out = render(&findings);
        let v = parse(&out);
        assert_eq!(v["summary"]["total"], 4);
        assert_eq!(v["summary"]["must_fix"], 2);
        assert_eq!(v["summary"]["should_fix"], 1);
        assert_eq!(v["summary"]["consider"], 1);
    }

    #[test]
    fn output_ends_with_newline() {
        let out = render(&[]);
        assert!(out.ends_with('\n'), "expected trailing newline");
    }

    #[test]
    fn consider_finding_omits_fix_field_on_wire() {
        let findings = vec![finding("nvim/plug-mapping", Severity::Consider, 10)];
        let out = render(&findings);
        let v = parse(&out);
        // Consider finding has fix: None -> should be absent (serde
        // skip_serializing_if from PA-2).
        assert!(
            v["findings"][0].get("fix").is_none(),
            "Consider severity should not carry a `fix` field on the wire"
        );
    }
}
