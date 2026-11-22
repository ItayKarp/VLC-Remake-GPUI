//! The log file's format (S5.3.4): tracing's usual plain text, or one JSON object per line for
//! tools that read logs. The choice can change while the app runs.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::{Format, Writer};
use tracing_subscriber::fmt::time::{FormatTime, SystemTime};
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

use super::redact_home;

/// How the log file is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// A line of text per event.
    #[default]
    Plain,
    /// A JSON object per line: `time`, `level`, `target`, `message` and any other fields.
    Json,
}

/// Whether the file is written as JSON, mirrored here so logging never needs the settings.
static JSON: AtomicBool = AtomicBool::new(false);

/// Switches the log file's format.
pub(super) fn set(format: LogFormat) {
    JSON.store(format == LogFormat::Json, Ordering::Relaxed);
}

/// Formats file events as [`set`] last chose.
pub(super) struct FileFormat {
    plain: Format,
}

impl FileFormat {
    /// Plain text until [`set`] says otherwise.
    pub(super) fn new() -> Self {
        Self { plain: Format::default() }
    }
}

impl<S, N> FormatEvent<S, N> for FileFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        cx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        if !JSON.load(Ordering::Relaxed) {
            return self.plain.format_event(cx, writer, event);
        }
        let mut time = String::new();
        SystemTime.format_time(&mut Writer::new(&mut time))?;
        writeln!(writer, "{}", json_line(&time, event))
    }
}

/// The event as a JSON object, its text with the home directory replaced by `~`. Redacting here
/// matters on Windows, where JSON doubles the backslashes the file's redactor looks for.
fn json_line(time: &str, event: &Event<'_>) -> Value {
    let metadata = event.metadata();
    let mut object = Map::new();
    object.insert("time".into(), time.into());
    object.insert("level".into(), metadata.level().as_str().into());
    object.insert("target".into(), metadata.target().into());
    event.record(&mut Fields(&mut object));
    Value::Object(object)
}

/// Collects an event's fields into a JSON object.
struct Fields<'a>(&'a mut Map<String, Value>);

impl Visit for Fields<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), redact_home(value).into());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().into(), value.into());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().into(), value.into());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().into(), value.into());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.0.insert(field.name().into(), value.into());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record_str(field, &format!("{value:?}"));
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::fmt::MakeWriter;

    use super::*;

    /// Collects what the layer writes.
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Captured {
        type Writer = Self;

        fn make_writer(&'a self) -> Self {
            self.clone()
        }
    }

    #[test]
    fn json_lines_hold_the_level_target_message_and_fields() {
        let captured = Captured::default();
        let subscriber = tracing_subscriber::fmt()
            .event_format(FileFormat::new())
            .with_writer(captured.clone())
            .finish();
        set(LogFormat::Json);
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "vlc::test", count = 3, "disk \"full\"");
        });
        set(LogFormat::Plain);
        let text = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        let line: Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(line["level"], "WARN");
        assert_eq!(line["target"], "vlc::test");
        assert_eq!(line["message"], "disk \"full\"");
        assert_eq!(line["count"], 3);
        assert!(line["time"].as_str().is_some_and(|time| !time.is_empty()));
    }
}
