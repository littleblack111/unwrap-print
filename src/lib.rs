#![deny(missing_docs)]

//! A tiny crate that provides ergonomics for printing and inspecting `Result` /
//! `Option` values.
//!
//! This crate exposes a small trait, `PrintableResult`, which provides an
//! `.unwrap_print()` method for `Result` and `Option` types. When an error is
//! encountered the value will be formatted and passed to a configurable global
//! printer function.
//!
//! The default printer simply forwards to `std::println!`. Embedded users can
//! install their own printer early in program startup using
//! [`try_set_printer`] (or `set_printer`).

extern crate alloc;

use alloc::fmt::format;
use core::fmt::{Arguments, Debug};
#[cfg(feature = "track-caller")]
use core::panic::Location;
use std::sync::Mutex;

static PRINTER: Mutex<Option<fn(&str)>> = Mutex::new(None);

fn default_printer(s: &str) {
    // Default behaviour: print to std output.
    // Using `println!` directly to include a trailing newline to match most
    // logging behavior.
    println!(
        "{}",
        s
    );
}

/// Attempt to set the global printer. This will succeed only once; subsequent
/// calls will return `false`.
///
/// This is intentionally conservative: install your printer early during
/// program initialization
/// Returns `true` if the printer was set successfully.
pub fn try_set_printer(printer: fn(&str)) -> bool {
    let mut guard = PRINTER
        .lock()
        .unwrap();
    if guard.is_some() {
        false
    } else {
        *guard = Some(printer);
        true
    }
}

/// Convenience wrapper which tries to set the printer and will overwrite any
/// previously configured one (useful for tests). This uses interior mutability
/// and is marked `pub(crate)` so production code prefers `try_set_printer`.
#[doc(hidden)]
pub fn set_printer_force(printer: fn(&str)) {
    // For tests and special use cases we allow replacing the global printer.
    let mut guard = PRINTER
        .lock()
        .unwrap();
    *guard = Some(printer);
}

#[cfg(test)]
#[doc(hidden)]
pub(crate) fn reset_printer() {
    let mut guard = PRINTER
        .lock()
        .unwrap();
    *guard = None;
}

/// Print an `Arguments` value using the configured printer.
///
/// This helper takes care of formatting the arguments into a `String` and
/// forwarding them to the currently installed printer (or the default
/// printer if none is installed).
pub fn print(args: Arguments<'_>) {
    // Format into a String - keeps the public API ergonomic and avoids forcing
    // consumers to worry about formatting internals.
    // Use `std::fmt::format` explicitly to ensure the correct function is used.
    let s = format(args);

    // Acquire the printer while holding the lock briefly, then drop the lock
    // before invoking the printer. This avoids potential deadlocks if the
    // installed printer calls back into this crate or attempts to acquire other
    // synchronization primitives that could conflict with the mutex held here.
    let maybe_printer = {
        let guard = PRINTER
            .lock()
            .unwrap();
        *guard
    };

    if let Some(printer) = maybe_printer {
        (printer)(&s);
    } else {
        default_printer(&s);
    }
}

/// Trait providing `.unwrap_print()` ergonomics.
///
/// The method returns the original `Result`/`Option` as `Result`, printing a
/// human readable message when an error/`None` is encountered.
pub trait PrintableResult<T, E> {
    /// Convert into `Result<T, E>`, printing any error encountered.
    fn unwrap_print(self) -> Result<T, E>;
}

impl<T, E: Debug> PrintableResult<T, E> for Result<T, E> {
    #[cfg_attr(
        feature = "track-caller",
        track_caller
    )]
    fn unwrap_print(self) -> Result<T, E> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => {
                #[cfg(feature = "track-caller")]
                {
                    let caller = Location::caller();
                    print(
                        format_args!(
                            "Error at {}:{}:{}: {e:#?}",
                            caller.file(),
                            caller.line(),
                            caller.column()
                        ),
                    );
                }
                #[cfg(not(feature = "track-caller"))]
                {
                    print(format_args!("Error: {e:#?}"));
                }
                Err(e)
            }
        }
    }
}

impl<T> PrintableResult<T, ()> for Option<T> {
    #[cfg_attr(
        feature = "track-caller",
        track_caller
    )]
    fn unwrap_print(self) -> Result<T, ()> {
        match self {
            Some(v) => Ok(v),
            None => {
                #[cfg(feature = "track-caller")]
                {
                    let caller = Location::caller();
                    print(
                        format_args!(
                            "Error at {}:{}:{}: Option::None",
                            caller.file(),
                            caller.line(),
                            caller.column()
                        ),
                    );
                }
                #[cfg(not(feature = "track-caller"))]
                {
                    print(format_args!("Error: Option::None"));
                }
                Err(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::{Mutex as StdMutex, OnceLock as StdOnceLock},
    };
    static TEST_MUTEX: StdMutex<()> = StdMutex::new(());

    #[test]
    fn try_set_printer_returns_true_then_false() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap();
        reset_printer();
        fn p(_: &str) {}
        assert!(try_set_printer(p));
        assert!(!try_set_printer(p));
    }

    #[test]
    fn set_printer_force_overwrites() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap();
        reset_printer();
        static FIRST: StdOnceLock<StdMutex<Vec<String>>> = StdOnceLock::new();
        FIRST
            .set(StdMutex::new(Vec::new()))
            .unwrap();
        fn first(s: &str) {
            let mutex = FIRST
                .get()
                .unwrap();
            mutex
                .lock()
                .unwrap()
                .push(s.to_string());
        }
        set_printer_force(first);
        print(format_args!("hello"));
        {
            let mutex = FIRST
                .get()
                .unwrap();
            let v = mutex
                .lock()
                .unwrap();
            assert_eq!(
                v.as_slice(),
                &["hello"]
            );
        }
        static SECOND: StdOnceLock<StdMutex<Vec<String>>> = StdOnceLock::new();
        SECOND
            .set(StdMutex::new(Vec::new()))
            .unwrap();
        fn second(s: &str) {
            let mutex = SECOND
                .get()
                .unwrap();
            mutex
                .lock()
                .unwrap()
                .push(s.to_string());
        }
        set_printer_force(second);
        print(format_args!("world"));
        {
            let v = SECOND
                .get()
                .unwrap()
                .lock()
                .unwrap();
            assert_eq!(
                v.as_slice(),
                &["world"]
            );
        }
    }

    // The previous implementation tried to capture stdout by swapping file
    // descriptors. That approach can be fragile in certain test runners. Here
    // we replace it with a child-process based test which runs the same test
    // executable in a subprocess and captures its stdout reliably.

    #[test]
    fn default_printer_writes_to_stdout() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap();
        reset_printer();
        // If we're the child process (indicated via env), do the printing and
        // exit immediately. Exiting prevents the test harness from running the
        // rest of the suite in the child process.
        if std::env::var("UNWRAP_PRINT_DEFAULT_PRINTER_CHILD").is_ok() {
            default_printer("foobar");
            std::io::stdout()
                .flush()
                .ok();
            std::process::exit(0);
        }

        // Otherwise spawn the current executable as a child with the env var set.
        // Capture its stdout and assert the default printer produced the
        // expected output.
        let exe = std::env::current_exe().expect("failed to find current exe");
        let out = std::process::Command::new(exe)
            .arg("default_printer_writes_to_stdout")
            .arg("--nocapture")
            .env(
                "UNWRAP_PRINT_DEFAULT_PRINTER_CHILD",
                "1",
            )
            .output()
            .expect("failed to spawn child process");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("foobar"),
            "child stdout did not contain expected text; stdout was: {stdout:?}"
        );
    }

    #[test]
    fn print_uses_installed_printer() {
        // Ensure no custom printer is installed.
        reset_printer();

        // Use a OnceLock + AtomicBool to detect whether the installed printer was
        // invoked. This avoids spawning subprocesses and is deterministic across
        // different test harness configurations.
        static INSTALLED_CALLED: std::sync::OnceLock<std::sync::atomic::AtomicBool> = std::sync::OnceLock::new();
        let flag = INSTALLED_CALLED.get_or_init(|| std::sync::atomic::AtomicBool::new(false));
        flag.store(
            false,
            std::sync::atomic::Ordering::SeqCst,
        );

        fn installed(_s: &str) {
            let f = INSTALLED_CALLED
                .get()
                .expect("flag should be initialized");
            f.store(
                true,
                std::sync::atomic::Ordering::SeqCst,
            );
        }

        set_printer_force(installed);
        print(format_args!("captured"));

        assert!(
            INSTALLED_CALLED
                .get()
                .expect("flag set")
                .load(std::sync::atomic::Ordering::SeqCst),
            "installed printer was not invoked"
        );
    }

    #[test]
    fn unwrap_print_result_err_prints_debug() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap();
        reset_printer();
        static CAP_ERR: StdOnceLock<StdMutex<Vec<String>>> = StdOnceLock::new();
        CAP_ERR
            .set(StdMutex::new(Vec::new()))
            .unwrap();
        fn cap_err(s: &str) {
            let m = CAP_ERR
                .get()
                .unwrap();
            m.lock()
                .unwrap()
                .push(s.to_string());
        }
        set_printer_force(cap_err);
        let res = Err::<(), _>(String::from("boom")).unwrap_print();
        assert!(res.is_err());
        let v = CAP_ERR
            .get()
            .unwrap()
            .lock()
            .unwrap();
        assert!(
            v.iter()
                .any(|s| s.contains("Error") && s.contains("\"boom\"")),
            "unexpected printed messages: {:?}",
            *v
        );
    }

    #[test]
    fn unwrap_print_option_none_prints() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap();
        reset_printer();
        static CAP_OPT: StdOnceLock<StdMutex<Vec<String>>> = StdOnceLock::new();
        CAP_OPT
            .set(StdMutex::new(Vec::new()))
            .unwrap();
        fn cap_opt(s: &str) {
            let m = CAP_OPT
                .get()
                .unwrap();
            m.lock()
                .unwrap()
                .push(s.to_string());
        }
        set_printer_force(cap_opt);
        let res = Option::<i32>::None.unwrap_print();
        assert!(res.is_err());
        let v = CAP_OPT
            .get()
            .unwrap()
            .lock()
            .unwrap();
        assert!(
            v.iter()
                .any(|s| s.contains("Option::None") || s.contains("Option::None")),
            "unexpected printed messages: {:?}",
            *v
        );
    }

    // Track-caller specific tests: only compile when the feature is enabled.
    #[cfg(feature = "track-caller")]
    #[test]
    fn track_caller_result_prints_location() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap();
        reset_printer();
        static CAP_TC_RES: StdOnceLock<StdMutex<String>> = StdOnceLock::new();
        CAP_TC_RES
            .set(StdMutex::new(String::new()))
            .unwrap();
        fn cap_res(s: &str) {
            let m = CAP_TC_RES
                .get()
                .unwrap();
            let mut lock = m
                .lock()
                .unwrap();
            lock.clear();
            lock.push_str(s);
        }
        set_printer_force(cap_res);
        let _ = Err::<(), _>(String::from("boom")).unwrap_print();
        let s = CAP_TC_RES
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(s.contains("Error at "));
        assert!(s.contains("\"boom\""));
    }

    #[cfg(feature = "track-caller")]
    #[test]
    fn track_caller_option_prints_location() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap();
        reset_printer();
        static CAP_TC_OPT: StdOnceLock<StdMutex<String>> = StdOnceLock::new();
        CAP_TC_OPT
            .set(StdMutex::new(String::new()))
            .unwrap();
        fn cap_opt_tc(s: &str) {
            let m = CAP_TC_OPT
                .get()
                .unwrap();
            let mut lock = m
                .lock()
                .unwrap();
            lock.clear();
            lock.push_str(s);
        }
        set_printer_force(cap_opt_tc);
        let _ = Option::<i32>::None.unwrap_print();
        let s = CAP_TC_OPT
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(s.contains("Error at "));
        assert!(s.contains("Option::None"));
    }
}
