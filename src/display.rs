//! The window: whether it fills the screen, and whether frames wait for the screen.
//!
//! `main.rs` opens the window as `DisplaySettings` says, and a change to them afterwards is
//! carried to the window here, and to `frame_pacing`, which paces the clock only while
//! frames wait for the screen (see there why).
//!
//! Works in a headless app, where there is no window and nothing to do.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode};

use crate::frame_pacing::FramePacing;

pub struct DisplayPlugin;

impl Plugin for DisplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DisplaySettings>().add_systems(
            Update,
            apply_settings.run_if(resource_changed::<DisplaySettings>),
        );
    }
}

/// Choices about the window. Insert it before adding `DisplayPlugin`, or change it at any
/// time.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct DisplaySettings {
    pub screen: ScreenMode,
    pub vsync: Vsync,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenMode {
    /// The whole screen, with the screen's own video mode: the fastest way to show a frame.
    #[default]
    Fullscreen,
    /// A window without a border the size of the screen. Quicker to switch away from.
    Borderless,
    /// A window with a border, of the size the operating system gives it.
    Windowed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Vsync {
    /// Frames are shown as soon as they are done, and tear. For measuring how much time
    /// there is to spare.
    Off,
    /// Frames wait for the screen, and one that just missed a refresh is shown at once,
    /// which can tear the top of the picture (`PresentMode::AutoVsync`).
    #[default]
    On,
    /// Frames wait for the screen, and one that missed a refresh waits for the next and
    /// never tears (`PresentMode::Fifo`).
    Strict,
}

impl DisplaySettings {
    pub fn window_mode(&self) -> WindowMode {
        match self.screen {
            ScreenMode::Fullscreen => {
                WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current)
            }
            ScreenMode::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
            ScreenMode::Windowed => WindowMode::Windowed,
        }
    }

    pub fn present_mode(&self) -> PresentMode {
        match self.vsync {
            Vsync::Off => PresentMode::AutoNoVsync,
            Vsync::On => PresentMode::AutoVsync,
            Vsync::Strict => PresentMode::Fifo,
        }
    }
}

fn apply_settings(
    settings: Res<DisplaySettings>,
    // Absent in a headless app.
    window: Option<Single<&mut Window, With<PrimaryWindow>>>,
    pacing: Option<ResMut<FramePacing>>,
) {
    if let Some(mut window) = window {
        let (mode, present_mode) = (settings.window_mode(), settings.present_mode());
        // Only a real change: setting the mode again would make the window flicker.
        if window.mode != mode {
            window.mode = mode;
        }
        if window.present_mode != present_mode {
            window.present_mode = present_mode;
        }
    }
    if let Some(mut pacing) = pacing {
        let on = settings.vsync != Vsync::Off;
        if pacing.on != on {
            pacing.on = on;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_what_the_game_always_opened_with() {
        let settings = DisplaySettings::default();
        assert_eq!(settings.present_mode(), PresentMode::AutoVsync);
        assert!(matches!(settings.window_mode(), WindowMode::Fullscreen(..)));
    }
}
