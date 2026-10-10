//! A track as plain data: the ground (`HeightGrid`), the route round it (`Course`), its
//! checkpoint gates, starting grid, textures, scenery, skies and backdrop (`TrackData`).

mod course;
mod data;
mod height_grid;
mod settle;

pub use course::{Course, Nearest};
pub use data::*;
pub use height_grid::{Diagonals, HeightGrid};
pub use settle::settle;
