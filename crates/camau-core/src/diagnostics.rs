#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct IssueData {
    pub path: String,
    pub code: String,
    pub message: String,
}

impl IssueData {
    pub fn new(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone)]
pub struct AssessmentData {
    pub issues: Vec<IssueData>,
}

impl AssessmentData {
    pub fn new(mut issues: Vec<IssueData>) -> Self {
        issues.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.code.cmp(&right.code))
        });
        issues.dedup_by(|left, right| left.path == right.path && left.code == right.code);
        Self { issues }
    }

    pub fn valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn to_text(&self) -> String {
        if self.valid() {
            return "Camau assessment valid\n".to_owned();
        }
        self.issues
            .iter()
            .map(|issue| {
                format!(
                    "{} {}: {}\n",
                    issue.code,
                    if issue.path.is_empty() {
                        "/"
                    } else {
                        &issue.path
                    },
                    one_line(&issue.message)
                )
            })
            .collect()
    }

    pub fn to_junit_xml(&self) -> String {
        if self.valid() {
            return "<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuite name=\"camau-assessment\" tests=\"1\" failures=\"0\"><testcase name=\"valid\"/></testsuite>".to_owned();
        }
        let mut output = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuite name=\"camau-assessment\" tests=\"{}\" failures=\"{}\">",
            self.issues.len(),
            self.issues.len()
        );
        for issue in &self.issues {
            output.push_str(&format!(
                "<testcase name=\"{} {}\"><failure type=\"{}\" message=\"{}\">{}</failure></testcase>",
                xml(&issue.code),
                xml(&issue.path),
                xml(&issue.code),
                xml(&issue.message),
                xml(&issue.message)
            ));
        }
        output.push_str("</testsuite>");
        output
    }

    pub fn to_github_annotations(&self, source: Option<&str>) -> String {
        self.issues
            .iter()
            .map(|issue| {
                let source = source
                    .map(|value| format!(",file={}", github(value)))
                    .unwrap_or_default();
                format!(
                    "::error title={}{}::{} at {}\n",
                    github(&issue.code),
                    source,
                    github(&issue.message),
                    github(if issue.path.is_empty() {
                        "/"
                    } else {
                        &issue.path
                    })
                )
            })
            .collect()
    }
}

fn one_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn github(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(':', "%3A")
        .replace(',', "%2C")
}
