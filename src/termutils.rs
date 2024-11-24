use std::io;

use crossterm::{
    cursor,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand as _,
};
use log::error;

#[derive(thiserror::Error, Debug)]
pub enum SetupError<T> {
    #[error("Error during terminal Setup: {0}")]
    SetupError(io::Error),

    #[error("Application Error: {0}")]
    ApplicationError(#[from] T),
}
macro_rules! with_cleanup {
    (cleanup: $cleanup:block, code: $code:block) => {{
        let f = move || $code;
        let res = f();
        $cleanup;
        res
    }};
}

pub fn with_setup_terminal<F, T, E>(f: F) -> Result<T, SetupError<E>>
where
    F: FnOnce() -> Result<T, E>,
{
    io::stdout()
        .execute(EnterAlternateScreen)
        .map_err(|e| SetupError::SetupError(e))?;
    with_cleanup!(
        cleanup: {
            if io::stdout().execute(LeaveAlternateScreen).is_err(){
                error!("Couldn't leave alt screen");
            }
        },
        code: {
            enable_raw_mode().map_err(|e| SetupError::SetupError(e))?;
            with_cleanup!(
                cleanup: {
                    if disable_raw_mode().is_err() {
                        error!("Couldn't disable raw mode");
                    }
                },
                code: {
                    io::stdout().execute(cursor::Hide).map_err(|e| SetupError::SetupError(e))?;
                    with_cleanup!(
                        cleanup: {
                            if io::stdout().execute(cursor::Show).is_err() {
                                error!("Couldn't hide cursor");
                            }
                        },
                        code: {
                            Ok(f()?)
                        }

                    )
                }
            )

        }
    )
}
