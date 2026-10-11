//! The scene depth (`scene_depth`) in an app that cannot draw: nothing is made, and the
//! slices that read it manage without it.

use bevy::prelude::*;
use monster_truck_rural_ruckus::scene_depth::{SceneDepth, SceneDepthPlugin};

#[test]
fn without_a_renderer_there_is_no_image() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, SceneDepthPlugin));
    app.update();
    assert!(app.world().get_resource::<SceneDepth>().is_none());
}
