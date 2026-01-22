use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{Subscriber, span};
use tracing_subscriber::{Layer, layer::Context};

/// A tracing layer that only logs spans that exceed a minimum duration
pub struct DurationFilterLayer {
    min_duration: Duration,
    span_times: Arc<Mutex<HashMap<span::Id, Instant>>>,
}

impl DurationFilterLayer {
    pub fn new(min_duration: Duration) -> Self {
        Self {
            min_duration,
            span_times: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl<S> Layer<S> for DurationFilterLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_enter(&self, id: &span::Id, _ctx: Context<'_, S>) {
        self.span_times
            .lock()
            .unwrap()
            .insert(id.clone(), Instant::now());
    }

    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        if let Some(start_time) = self.span_times.lock().unwrap().remove(&id) {
            let elapsed = start_time.elapsed();

            if elapsed >= self.min_duration {
                // Only log if duration exceeds threshold
                if let Some(span) = ctx.span(&id) {
                    let metadata = span.metadata();

                    // Get span fields if any
                    let extensions = span.extensions();
                    let fields_str = if let Some(fields) = extensions
                        .get::<tracing_subscriber::fmt::FormattedFields<
                        tracing_subscriber::fmt::format::DefaultFields,
                    >>() {
                        if !fields.fields.is_empty() {
                            format!(" {}", fields.fields)
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    // Use eprintln! directly to avoid escaping the pre-formatted ANSI codes in fields
                    let name = metadata.name();
                    let target = metadata.target();
                    eprintln!(
                        "\x1b[38;5;208m⚠️  [SLOW] ⚠️\x1b[0m  {target}::{name} took \x1b[36m{elapsed:?}\x1b[0m (threshold: {:?}){fields_str}",
                        self.min_duration
                    );
                }
            }
        }
    }
}
