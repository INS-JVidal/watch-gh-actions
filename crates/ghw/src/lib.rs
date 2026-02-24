#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::struct_excessive_bools,
    clippy::wildcard_imports,
    clippy::too_many_lines,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::fn_params_excessive_bools,
    clippy::too_many_arguments,
    clippy::doc_markdown
)]

pub mod cli;
pub mod executor;
pub mod parser;

// Re-export ciw_core modules for backward compatibility with tests
pub use ciw_core::app;
pub use ciw_core::diff;
pub use ciw_core::events;
pub use ciw_core::input;
pub use ciw_core::notify;
pub use ciw_core::tui;
