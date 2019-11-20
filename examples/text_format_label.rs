//! Print metrics to stderr with custom formatter including a label.

extern crate dipstick;

use dipstick::{AppLabel, Formatting, InputKind, InputScope, LabelOp, LineFormat, LineOp, LineTemplate, MetricName, Stream, Locking};
use std::thread::sleep;
use std::time::Duration;

/// Generates template like "$METRIC $value $label_value["abc"]\n"
struct MyFormat;

impl LineFormat for MyFormat {
    fn template(&self, name: &MetricName, _kind: InputKind) -> LineTemplate {
        vec![
            LineOp::Literal(format!("{} ", name.join(".")).to_uppercase().into()),
            LineOp::ValueAsText,
            LineOp::Literal(" ".into()),
            LineOp::LabelExists(
                "abc".into(),
                vec![
                    LabelOp::LabelKey,
                    LabelOp::Literal(":".into()),
                    LabelOp::LabelValue,
                ],
            ),
            LineOp::NewLine,
        ]
        .into()
    }
}

fn main() {
    let counter = Stream::to_stderr()
        .formatting(MyFormat)
        .locking()
        .counter("counter_a");
    AppLabel::set("abc", "xyz");
    loop {
        // report some metric values from our "application" loop
        counter.count(11);
        sleep(Duration::from_millis(500));
    }
}
