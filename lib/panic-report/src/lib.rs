#![doc = include_str!("../README.md")]

use std::backtrace::Backtrace;
use std::collections::BTreeMap;
use std::panic::PanicHookInfo;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use uuid::Uuid;

/// Shared map of serializable context information to include in the report
type Context = BTreeMap<String, toml::Value>;
static CONTEXT: Mutex<Option<Context>> = Mutex::new(Some(Context::new()));

/// Macro to setup the panic report hook
///
/// # Examples
///
/// ```
/// // Get details from cargo
/// panic_report::setup!();
/// ```
///
/// ```
/// // Use custom details
/// panic_report::setup! {
///   name: "example_crate",
///   version: "0.0.1",
///   homepage: "https://turbo.fish",
///   contacts: []
/// }
/// ```
#[macro_export]
macro_rules! setup {
    () => {$crate::setup!{
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        homepage: env!("CARGO_PKG_HOMEPAGE"),
        contacts: {
            const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
            if AUTHORS.is_empty() {
                vec![]
            } else {
                AUTHORS.split(':').map(|a| a.to_string()).collect()
            }
        }
    }};

    {
        name: $name:expr,
        version: $version:expr,
        homepage: $homepage:expr,
        contacts: $contacts:expr$(,)?
    } => {
        #[cfg(not(debug_assertions))]
        {
            if std::env::var("RUST_BACKTRACE").ok().is_none() {
                std::panic::set_hook(Box::new(|info: &std::panic::PanicInfo| {
                    let backtrace = std::backtrace::Backtrace::force_capture();
                    let report = $crate::Report::new(info, &backtrace, $name, $version);
                    $crate::print_default_panic(
                        $name,
                        $homepage,
                        $contacts,
                        &report.cause,
                        report.save()
                    );
                }));
            }
        }
    };
}

/// Utility to add context to the panic report at runtime.
/// Recommended to only use during the initialization phase of a program.
///
/// # Example
///
/// ```
/// use panic_report::{add_context, setup};
///
/// setup!();
/// add_context("extra_info", "foo");
/// add_context("extra_info_two", "bar");
/// ```
#[inline(always)]
pub fn add_context(_key: impl ToString, _value: impl serde::Serialize) {
    CONTEXT
        .lock()
        .expect("poisoned lock")
        .as_mut()
        .unwrap()
        .insert(
            _key.to_string(),
            toml::Value::try_from(_value).expect("failed to convert data to a toml value"),
        );
}

/// Print the default panic report message.
#[inline(always)]
pub fn print_default_panic<S: ToString>(
    name: &str,
    homepage: &str,
    contacts: impl AsRef<[S]>,
    cause: &str,
    report_path: PathBuf,
) {
    use std::io::Write;

    let mut stderr = anstream::stderr().lock();
    stderr
        .write_all(anstyle::AnsiColor::Red.render_fg().to_string().as_bytes())
        .ok();

    writeln!(
        stderr,
        "
Uh oh, `{name}` had a problem and crashed:

{cause}

A report has been saved to {report_path:?}.

To help diagnose the problem, you can submit a crash report as an issue or via email.
"
    )
    .ok();

    if !homepage.is_empty() {
        writeln!(stderr, "Homepage: {homepage}").ok();
    }

    let contacts = contacts.as_ref();
    if !contacts.is_empty() {
        if contacts.len() == 1 {
            writeln!(stderr, "Contact: {}", contacts[0].to_string()).ok();
        } else {
            writeln!(stderr, "Contacts:").ok();
            for contact in contacts {
                writeln!(stderr, "\t- {}", contact.to_string()).ok();
            }
        }
    }

    writeln!(
        stderr,
        "
We take privacy seriously, and do not automatically collect crash reports. 
Due to this, we rely on users to submit reports.

Thank you kindly!
",
    )
    .ok();

    stderr
        .write_all(anstyle::Reset.render().to_string().as_bytes())
        .ok();
}

/// Serializable panic report for more granular hook creation
///
/// # Example
///
/// ```
/// use std::backtrace::Backtrace;
/// use std::panic::PanicInfo;
///
/// use panic_report::Report;
///
/// std::panic::set_hook(Box::new(|info: &PanicInfo| {
///     let backtrace = Backtrace::force_capture();
///     let report = Report::new(info, &backtrace, "test-crate", "0.0.1");
///     let report_path = report.save();
///     eprintln!("Oops, we panicked! Report available at {report_path:?}");
/// }));
/// ```
#[derive(Serialize)]
pub struct Report {
    pub name: String,
    pub version: String,
    pub file: String,
    pub cause: String,
    pub timestamp: u64,
    pub backtrace: String,
    #[serde(skip_serializing_if = "Context::is_empty")]
    pub context: Context,
}

impl Report {
    /// Capture a new report with the information, context, and backtrace.
    #[inline(always)]
    pub fn new(info: &PanicHookInfo, backtrace: &Backtrace, name: &str, version: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cause = match (
            info.payload().downcast_ref::<&str>(),
            info.payload().downcast_ref::<String>(),
        ) {
            (Some(&s), _) => s.to_string(),
            (_, Some(s)) => s.to_string(),
            (None, None) => "unknown".to_string(),
        };

        let file = match info.location() {
            Some(location) => location.to_string(),
            None => "unknown location".to_string(),
        };

        let backtrace = backtrace.to_string();

        let context = CONTEXT.lock().unwrap().take().unwrap();

        Report {
            name: name.into(),
            version: version.into(),
            file,
            cause,
            timestamp,
            backtrace,
            context,
        }
    }

    /// Serialize as toml and save to a random temporary file.
    #[inline(always)]
    pub fn save(&self) -> PathBuf {
        let report_path =
            std::env::temp_dir().join(format!("report-{}.toml", Uuid::new_v4().hyphenated()));

        match toml::to_string_pretty(&self) {
            Ok(toml) => {
                if let Err(e) = std::fs::write(&report_path, toml) {
                    eprintln!("failed to write panic report to disk: {e:?}");
                }
            },
            Err(e) => {
                eprintln!("failed to encode panic report: {e:?}");
                panic!();
            },
        }

        report_path
    }
}
