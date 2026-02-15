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

pub mod app;
pub mod cli;
pub mod diff;
pub mod events;
pub mod gh;
pub mod input;
pub mod notify;
pub mod tui;
