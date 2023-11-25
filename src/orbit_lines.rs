use bevy::{prelude::{App, Entity, Gizmos, in_state, IntoSystemConfigs, Plugin, Query, Res, ResMut, Resource, Transform, Update, Vec3, With, Without, PostUpdate, PreUpdate, Component}, time::{Time, Timer, TimerMode}};

use crate::{body::{BodyChildren, Moon, OrbitSettings, Planet, SimPosition, Star}, constants::M_TO_UNIT, physics::{apply_physics, SubSteps, Pause}, SimState, apsis::ApsisBody, speed::Speed};

pub struct OrbitLinePlugin;

impl Plugin for OrbitLinePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<OrbitOffset>()
            .add_systems(PreUpdate, (update_lines.after(apply_physics), draw_orbit_line.after(update_lines), clean_orbit_lines.after(draw_orbit_line)).run_if(in_state(SimState::Simulation)));
    }
}

#[derive(Resource)]
pub struct OrbitOffset {
    
    pub value: Vec3,
    pub enabled: bool,
    
}

impl Default for OrbitOffset {
    
    fn default() -> Self {
        OrbitOffset {
            value: Vec3::ZERO,
            enabled: true,
        }
    }
    
}

const MULTIPLIER: f32 = 0.0001;

fn update_lines(
    mut planet_query: Query<(&mut OrbitSettings, &SimPosition, &BodyChildren, With<Planet>, Without<Moon>, Without<Star>)>,
    mut moon_query: Query<(Entity, &SimPosition, &mut OrbitSettings, With<Moon>, Without<Planet>, Without<Star>)>,
    time: Res<Time>,
    speed: Res<Speed>,
    substeps: Res<SubSteps>,
    pause: Res<Pause>
) {
    if pause.0 {
        return;
    }
    for (mut orbit, pos, _, _, _, _) in &mut planet_query {
        if orbit.draw_lines {
            let speed = speed.0 as f32 * (substeps.0 as f32);
            let max_step = (orbit.period as f32 / speed) * MULTIPLIER;
            if orbit.step >= max_step {
                orbit.lines.push((pos.0 * M_TO_UNIT).as_vec3());
                orbit.step = 0.0;
            } else {
                orbit.step += time.delta_seconds();
            }
        }
    }
    for (entity, pos, mut orbit, _, _, _) in &mut moon_query {
        if orbit.draw_lines {
            if let Some((_, p_pos, _, _, _, _)) = planet_query.iter().find(|(_, _, children, _, _, _)| {
                children.0.contains(&entity)
            }) {
                let speed = speed.0 as f32 * (substeps.0 as f32);
                let max_step = (orbit.period as f32 / speed) * MULTIPLIER;
                if orbit.step >= max_step {
                    let raw_p_pos = (p_pos.0 * M_TO_UNIT).as_vec3();
                    let raw_pos = (pos.0 * M_TO_UNIT).as_vec3();
                    orbit.lines.push(raw_pos - raw_p_pos);   
                    orbit.step = 0.0;
                } else {
                    orbit.step += time.delta_seconds();
                }
            }
        }
    }
}

fn draw_orbit_line(
    offset: Res<OrbitOffset>,
    planet_query: Query<(&OrbitSettings, &SimPosition, &BodyChildren, &Transform, With<Planet>, Without<Moon>, Without<Star>)>,
    moon_query: Query<(Entity, &OrbitSettings, &Transform, With<Moon>, Without<Planet>, Without<Star>)>,
    mut gizmos: Gizmos
) {
    for (orbit, _, _, transform, _, _, _) in &planet_query {
        if orbit.draw_lines {
            draw_lines(orbit, offset.value, &mut gizmos, transform.translation)
        }
    }
    for (entity, orbit, transform, _, _, _) in &moon_query {
        if orbit.draw_lines {
            if let Some((_, p_pos, _, _, _, _, _)) = planet_query.iter().find(|(_, _, children, _, _, _, _)| {
                children.0.contains(&entity)
            }) {
                let raw_p_pos = (p_pos.0 * M_TO_UNIT).as_vec3();
                draw_lines(orbit, offset.value + raw_p_pos, &mut gizmos, transform.translation)
            }
        }
    }
}

pub fn draw_lines(orbit: &OrbitSettings, offset: Vec3, gizmos: &mut Gizmos, current_pos: Vec3) {
    for (index, first) in orbit.lines.iter().enumerate() {
        if let Some(second) = orbit.lines.get(index + 1) {
            gizmos.line(*first + offset, *second + offset, orbit.color);
        }
    }
    if let Some(last) = orbit.lines.last() {
        gizmos.line(*last + offset, current_pos, orbit.color)   
    }
}

fn clean_orbit_lines(
    mut bodies: Query<(&mut OrbitSettings, &Transform)>,
) {
    for (mut orbit, _) in &mut bodies {
        if orbit.draw_lines {
            let amount = orbit.lines.len() as i32;
            if amount > orbit.max_points {
                while orbit.lines.len() as i32 >= orbit.max_points {
                    orbit.lines.remove(0);
                }
            }
        } else {
            if !orbit.lines.is_empty() {
                orbit.lines.clear();
            }
        }
    }
}
