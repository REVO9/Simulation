use std::ops::Add;
use std::process::exit;
use bevy::app::{App, Plugin, Update};
use bevy::math::{Vec3, I64Vec3, Vec3A, DVec3};
use bevy::prelude::{in_state, IntoSystemConfigs, Mut, Query, Res, Resource, Time, Transform, Entity, GlobalTransform, BVec3, Gizmos, Color};
use crate::body::{Acceleration, Mass, SimPosition, Velocity, DrawOrbitLines, OrbitLines, OrbitColor};
use crate::constants::{G, KM_TO_AU};
use crate::SimState;
use crate::selection::SelectedEntity;
use crate::speed::Speed;
use itertools::Itertools;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<Pause>()
            .register_type::<Velocity>()
            .register_type::<Acceleration>()
            .register_type::<Mass>()
            .register_type::<SimPosition>()
            .register_type::<DrawOrbitLines>()
            .register_type::<OrbitLines>()
            .add_systems(Update, (update_acceleration, update_velocity.after(update_acceleration), update_position.after(update_velocity)).run_if(in_state(SimState::Simulation)));
    }
}

#[derive(Resource, Default)]
pub struct Pause(pub bool);

fn update_acceleration(
    mut query: Query<(&Mass, &mut Acceleration, &SimPosition)>,
) {
    let mut other_bodies: Vec<(&Mass, Mut<Acceleration>, &SimPosition)> = Vec::new();
    for (mass, mut acc, sim_pos) in query.iter_mut() {
        acc.0 = DVec3::ZERO;
        for (other_mass, ref mut other_acc, other_sim_pos) in other_bodies.iter_mut() {
            let r_sq = (sim_pos.0 - other_sim_pos.0).length_squared() as f64;
            let force_direction = DVec3::from((other_sim_pos.0 - sim_pos.0).normalize()); // Calculate the direction vector  
            
            let force_magnitude = G * mass.0 * other_mass.0 / r_sq;
          //  println!("r_sq: {}", G * mass.0 * other_mass.0 / r_sq);            
            let force = force_direction * force_magnitude;
         //   println!("force: {}", force);            
            acc.0 += force;
            other_acc.0 -= force;
        }
        other_bodies.push((mass, acc, sim_pos));
    }
    for (mass, mut acc, _) in query.iter_mut() {
        acc.0 /= mass.0;
    }
}

fn update_velocity(
    mut query: Query<(&mut Velocity, &Acceleration)>,
    time: Res<Time>,
    speed: Res<Speed>
) {
    for (mut vel, acc) in query.iter_mut() {
        vel.0 += acc.0 * time.delta_seconds() as f64 * speed.0;
    }
}

pub fn update_position(
    mut query: Query<(Entity, &DrawOrbitLines, &mut OrbitLines, &OrbitColor, &mut SimPosition, &mut Transform, &Velocity)>,
    time: Res<Time>,
    speed: Res<Speed>,
    selected_entity: Res<SelectedEntity>,
    mut gizmos: Gizmos
) {
    let delta_time = time.delta_seconds() as f64;
    // Calculate the offset based on the selected entity's position
    let offset = match selected_entity.0 {
        Some(selected) => {
            if let Ok((_, draw_orbit_lines, mut orbit_lines, orbit_color, mut sim_pos, mut transform, vel)) = query.get_mut(selected) {
                sim_pos.0 += vel.0 * delta_time * speed.0; //this is the same step as below, but we are doing this first for the offset
                let raw_translation = sim_pos.0 * KM_TO_AU;
                transform.translation = Vec3::ZERO; //the selected entity will always be at 0,0,0
                if draw_orbit_lines.0 {
                    let raw_vec3 = Vec3::new(raw_translation.x as f32, raw_translation.y as f32, raw_translation.z as f32);
                    orbit_lines.0.push(raw_vec3);
                    draw_lines(&orbit_lines, raw_vec3, orbit_color.0, &mut gizmos)
                }
                -raw_translation 
            } else {
                DVec3::ZERO 
            }
        }
        None => DVec3::ZERO,
    };
    for (entity, draw_orbit_lines, mut orbit_lines, orbit_color, mut sim_pos, mut transform, vel) in query.iter_mut() {
        if let Some(s_entity) = selected_entity.0 {
            if s_entity == entity {
                continue;
            }
        }
        sim_pos.0 += vel.0 * delta_time * speed.0;
        let offset3: Vec3 = Vec3::new(offset.x as f32, offset.y as f32, offset.z as f32);
        let sim_pos3 = Vec3::new(sim_pos.0.x as f32, sim_pos.0.y as f32, sim_pos.0.z as f32);
        let pos_without_offset = sim_pos3 * KM_TO_AU as f32;
        transform.translation = pos_without_offset + offset3; //apply offset
        if draw_orbit_lines.0 {
            orbit_lines.0.push(pos_without_offset);
            draw_lines(&orbit_lines, -offset3, orbit_color.0, &mut gizmos)
        }
    }
}

fn draw_lines(lines: &OrbitLines, offset: Vec3, color: Color, gizmos: &mut Gizmos) {
    let points: Vec<&[Vec3]> = lines.0.chunks(2).collect();
    for outer in points {
        if let Some(first) = outer.get(0) {
            if let Some(second) = outer.get(1) {
                gizmos.line(*first - offset, *second - offset, color);
            }
        }
    }
}