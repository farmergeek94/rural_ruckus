//! Carries `EnvironmentSettings::lighting` to the materials.
//!
//! Off: every material that can be is drawn unlit, as its plain colour and texture, the
//! least a pixel can cost. The sun, the sky's light, the lamps and the shadows then light
//! nothing. Simple: the tile material (the ground, the scenery and the backdrop, most of
//! the picture) is lit the cheap way that `tiles.wgsl` has, by the sun, its shadows and
//! the ambient light alone, and everything else as it always is. Measured on integrated
//! graphics, drawing unlit saved a great deal; Simple has not been measured.
//!
//! For Off it turns off the lighting of the standard materials and of the tile material (the
//! ground, the scenery and the backdrop), each as it is made or changed, and turns it back
//! on for only those it turned off: some materials, such as the sky's and the particles',
//! are unlit by design. The water's shader lights it whatever its material says.
//!
//! Uses the `environment` slice, whose setting it is, and the `track` slice's tile
//! material. May be changed while racing. Works in a headless app, where there are no
//! materials.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::environment::{EnvironmentSettings, Lighting};
use crate::track::{TileLighting, TileMaterial};

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnvironmentSettings>()
            .init_resource::<TurnedOff>()
            // After everything that makes or changes a material in `Update`.
            .add_systems(PostUpdate, (light_standard_materials, light_tile_materials));
    }
}

/// The materials whose lighting this slice turned off, and only those, to be turned back
/// on.
#[derive(Resource, Default)]
struct TurnedOff {
    standard: HashSet<AssetId<StandardMaterial>>,
    tile: HashSet<AssetId<TileMaterial>>,
}

fn light_standard_materials(
    settings: Res<EnvironmentSettings>,
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut turned_off: ResMut<TurnedOff>,
) {
    let Some(mut materials) = materials else {
        events.clear();
        return;
    };
    let ids: Vec<AssetId<StandardMaterial>> = if settings.is_changed() {
        events.clear();
        materials.ids().collect()
    } else {
        events.read().filter_map(made_or_changed).collect()
    };
    for id in ids {
        let Some(unlit) = materials.get(id).map(|material| material.unlit) else {
            continue;
        };
        if let Some(wanted) = wanted(&settings, unlit, &mut turned_off.standard, id)
            && let Some(mut material) = materials.get_mut(id)
        {
            material.unlit = wanted;
        }
    }
}

fn light_tile_materials(
    settings: Res<EnvironmentSettings>,
    mut events: MessageReader<AssetEvent<TileMaterial>>,
    materials: Option<ResMut<Assets<TileMaterial>>>,
    mut turned_off: ResMut<TurnedOff>,
) {
    let Some(mut materials) = materials else {
        events.clear();
        return;
    };
    let ids: Vec<AssetId<TileMaterial>> = if settings.is_changed() {
        events.clear();
        materials.ids().collect()
    } else {
        events.read().filter_map(made_or_changed).collect()
    };
    let lighting = TileLighting {
        simple: (settings.lighting == Lighting::Simple) as u32,
    };
    for id in ids {
        let Some((unlit, was)) = materials
            .get(id)
            .map(|material| (material.base.unlit, material.extension.lighting))
        else {
            continue;
        };
        let unlit = wanted(&settings, unlit, &mut turned_off.tile, id);
        // Only a real change, which changes the material once more, and then it is as
        // wanted.
        if (unlit.is_some() || was != lighting)
            && let Some(mut material) = materials.get_mut(id)
        {
            if let Some(unlit) = unlit {
                material.base.unlit = unlit;
            }
            material.extension.lighting = lighting;
        }
    }
}

fn made_or_changed<A: Asset>(event: &AssetEvent<A>) -> Option<AssetId<A>> {
    match *event {
        AssetEvent::Added { id } | AssetEvent::Modified { id } => Some(id),
        _ => None,
    }
}

/// What material `id`'s `unlit` should become, or `None` to leave it, keeping
/// `turned_off` up to date. Lighting off turns off whatever is lit; lighting on turns
/// back on only what was turned off.
fn wanted<A: Asset>(
    settings: &EnvironmentSettings,
    unlit: bool,
    turned_off: &mut HashSet<AssetId<A>>,
    id: AssetId<A>,
) -> Option<bool> {
    if settings.lighting == Lighting::Off {
        // Again, if something has lit it since.
        if unlit {
            return None;
        }
        turned_off.insert(id);
        Some(true)
    } else {
        turned_off.remove(&id).then_some(false)
    }
}
