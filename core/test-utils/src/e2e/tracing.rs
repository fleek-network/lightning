use ansi_term::{Color, Style};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::TryInitError;
use tracing_subscriber::EnvFilter;

pub struct TracingOptions {
    pub with_console: bool,
}

#[allow(unused)]
pub fn try_init_tracing(options: Option<TracingOptions>) -> Result<(), TryInitError> {
    let env_filter = EnvFilter::builder()
        .from_env()
        .unwrap()
        .add_directive("anemo=warn".parse().unwrap())
        .add_directive("rustls=warn".parse().unwrap())
        .add_directive("h2=warn".parse().unwrap())
        .add_directive("tokio=warn".parse().unwrap())
        .add_directive("runtime=warn".parse().unwrap());
    let registry = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_thread_names(true)
            .event_format(CustomFormatter::default())
            .with_filter(env_filter),
    );
    if let Some(options) = options {
        if options.with_console {
            let console_layer = console_subscriber::Builder::default()
                .with_default_env()
                .server_addr(([0, 0, 0, 0], 6669))
                .spawn();
            registry.with(console_layer).try_init()?;
            return Ok(());
        }
    }
    registry.try_init()
}

#[allow(unused)]
pub fn init_tracing(options: Option<TracingOptions>) {
    try_init_tracing(options).expect("failed to initialize tracing");
}

#[allow(unused)]
pub fn try_init_tracing_with_tokio_console() -> Result<(), TryInitError> {
    try_init_tracing(Some(TracingOptions { with_console: true }))
}

#[allow(unused)]
pub fn init_tracing_with_tokio_console() {
    try_init_tracing_with_tokio_console().expect("failed to initialize tracing with tokio console");
}

/// Custom formatter for the tracing subscriber.
///
/// This formatter is similar to the default but does not add padding around each section.
struct CustomFormatter {
    ansi: bool,
}

impl Default for CustomFormatter {
    fn default() -> Self {
        Self { ansi: true }
    }
}

impl<S, N> FormatEvent<S, N> for CustomFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let metadata = event.metadata();

        // Write the timestamp with a dimmed style.
        let timestamp = chrono::Local::now();
        if self.ansi {
            write!(
                writer,
                "{} ",
                Style::new()
                    .dimmed()
                    .paint(timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            )?;
        } else {
            write!(writer, "{} ", timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ"))?;
        }

        // Write the log level with color.
        let level_style = match *metadata.level() {
            Level::TRACE => Color::Purple.normal(),
            Level::DEBUG => Color::Blue.normal(),
            Level::INFO => Color::Green.normal(),
            Level::WARN => Color::Yellow.normal(),
            Level::ERROR => Color::Red.normal(),
        };
        if self.ansi {
            write!(writer, "{} ", level_style.paint(metadata.level().as_str()))?;
        } else {
            write!(writer, "{} ", metadata.level())?;
        }

        // Write the thread name without dimming.
        if let Some(thread_name) = std::thread::current().name() {
            write!(writer, "{} ", thread_name)?;
        }

        // Write the additional token/part if available (e.g., span name) in brighter style.
        if let Some(scope) = ctx.event_scope() {
            if let Some(span) = scope.from_root().last() {
                if self.ansi {
                    write!(writer, "{} ", Style::new().bold().paint(span.name()))?;
                } else {
                    write!(writer, "{} ", span.name())?;
                }
            }
        }

        // Write the module path with a dimmed style.
        if let Some(module_path) = metadata.module_path() {
            if self.ansi {
                write!(writer, "{}: ", Style::new().dimmed().paint(module_path))?;
            } else {
                write!(writer, "{}: ", module_path)?;
            }
        }

        // Write the event message and fields.
        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}
