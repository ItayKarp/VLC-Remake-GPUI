//! The library's model (Stage 4): what a media folder's paths compare as ([`path_key`]) and how
//! far each media was watched ([`watch_state`]). Its tables live in [`db`](crate::db).

pub mod path_key;
pub mod watch_state;
