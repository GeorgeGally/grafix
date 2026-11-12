//! Minimal orbit camera: left-drag orbits, right/middle-drag pans, wheel
//! dollies. Input comes from Bevy's own canvas listeners, so it never competes
//! with the Svelte UI's pointer handling.

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
};

use crate::commands::ResetCameraRequest;

#[derive(Component, Clone, Copy, Debug)]
pub struct OrbitCamera {
    pub focus: Vec3,
    pub radius: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::new(0.0, 1.0, 0.0),
            radius: 12.0,
            yaw: 0.6,
            pitch: 0.28,
        }
    }
}

impl OrbitCamera {
    pub fn transform(&self) -> Transform {
        let rotation = Quat::from_euler(EulerRot::YXZ, self.yaw, -self.pitch, 0.0);
        let position = self.focus + rotation * Vec3::new(0.0, 0.0, self.radius);
        Transform::from_translation(position).looking_at(self.focus, Vec3::Y)
    }
}

pub(crate) struct OrbitCameraPlugin;

impl Plugin for OrbitCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, orbit_camera);
    }
}

fn orbit_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut reset: MessageReader<ResetCameraRequest>,
    mut cameras: Query<(&mut OrbitCamera, &mut Transform)>,
) {
    let reset_requested = reset.read().count() > 0;
    for (mut orbit, mut transform) in &mut cameras {
        if reset_requested {
            *orbit = OrbitCamera::default();
        }
        let delta = motion.delta;
        if buttons.pressed(MouseButton::Left) && delta != Vec2::ZERO {
            orbit.yaw -= delta.x * 0.005;
            orbit.pitch = (orbit.pitch + delta.y * 0.005).clamp(-1.45, 1.45);
        }
        if (buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle))
            && delta != Vec2::ZERO
        {
            let right = transform.right();
            let up = transform.up();
            let scale = orbit.radius * 0.0015;
            orbit.focus += (-delta.x * *right + delta.y * *up) * scale;
        }
        let lines = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        if lines != 0.0 {
            orbit.radius = (orbit.radius * (1.0 - lines * 0.08)).clamp(1.5, 60.0);
        }
        let next = orbit.transform();
        if *transform != next {
            *transform = next;
        }
    }
}
