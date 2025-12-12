use std::fmt::Debug;
#[cfg(feature = "track-caller")]
use std::panic::Location;

pub trait PrintableResult<T, E: Debug> {
    #[cfg(feature = "track-caller")]
    #[track_caller]
    fn unwrap_print(self) -> Result<T, E>;
}

impl<T, E: Debug> PrintableResult<T, E> for Result<T, E> {
    #[cfg(feature = "track-caller")]
    #[track_caller]
    fn unwrap_print(self) -> Result<T, E> {
        #[cfg(feature = "track-caller")]
        let caller = Location::caller();
        match self {
            Ok(value) => Ok(value),
            Err(err) => {
                if cfg!(feature = "track-caller") {
                    println!(
                        "Error at {}:{}:{}: {err:#?}",
                        caller.file(),
                        caller.line(),
                        caller.column()
                    );
                } else {
                    println!("Error: {err:#?}");
                }
                Err(err)
            }
        }
    }
}

impl<T> PrintableResult<T, ()> for Option<T> {
    #[cfg(feature = "track-caller")]
    #[track_caller]
    fn unwrap_print(self) -> Result<T, ()> {
        #[cfg(feature = "track-caller")]
        let caller = Location::caller();
        match self {
            Some(value) => Ok(value),
            None => {
                if cfg!(feature = "track-caller") {
                    println!(
                        "Error at {}:{}:{}: Option::None",
                        caller.file(),
                        caller.line(),
                        caller.column()
                    );
                } else {
                    println!("Error: Option::None");
                }
                Err(())
            }
        }
    }
}
