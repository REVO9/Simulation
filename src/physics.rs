use std::collections::VecDeque;
use std::time::{Instant, Duration};

use bevy::app::{App, Plugin, Update};
use bevy::diagnostic::{DiagnosticId, Diagnostics, RegisterDiagnostic, Diagnostic};
use bevy::math::{DVec3, Vec3};
use bevy::prelude::{Entity, in_state, IntoSystemConfigs, Mut, Query, Res, ResMut, Resource, Time, Transform};
use bevy::reflect::List;

use crate::body::{Acceleration, Mass, OrbitSettings, SimPosition, Velocity};
use crate::constants::{DEFAULT_SUB_STEPS, G, M_TO_UNIT};
use crate::orbit_lines::OrbitOffset;
use crate::selection::SelectedEntity;
use crate::SimState;
use crate::speed::Speed;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<Pause>()
            .init_resource::<SubSteps>()
            .init_resource::<NBodyStats>()
            .register_type::<Velocity>()
            .register_type::<Acceleration>()
            .register_type::<Mass>()
            .register_type::<SimPosition>()
            .register_type::<OrbitSettings>()
            .register_diagnostic(Diagnostic::new(NBODY_STEP_TIME, "nbody_step_time", 10))
            .register_diagnostic(Diagnostic::new(NBODY_TOTAL_TIME, "nbody_total_time", 10))
            .add_systems(Update, (apply_physics).run_if(in_state(SimState::Simulation)));
    }
}

#[derive(Resource, Default)]
pub struct Pause(pub bool);

#[derive(Resource)]
pub struct SubSteps(pub i32);

#[derive(Resource, Default)]
pub struct NBodyStats {
    
    pub steps: i32
             
}

impl Default for SubSteps {
    fn default() -> Self {
        SubSteps(DEFAULT_SUB_STEPS)
    }   
}

impl SubSteps {
    
    pub fn small_step_up(&mut self) {
        self.0 *= 2; 
    }
        
    pub fn big_step_up(&mut self) {
        self.0 *= 10;
    }
        
    pub fn small_step_down(&mut self) {
        self.0 = std::cmp::max(self.0 / 2, 1);
    }
        
    pub fn big_step_down(&mut self) {
        self.0 = std::cmp::max(self.0 / 10, 1);
    }
      
}

pub const NBODY_TOTAL_TIME: DiagnosticId =
    DiagnosticId::from_u128(337040787172757619024841343456040760896);
    
pub const NBODY_STEP_TIME: DiagnosticId =
    DiagnosticId::from_u128(337040787171757619024831343456040760892);

pub fn apply_physics(
    mut query: Query<(Entity, &Mass, &mut Acceleration, &mut OrbitSettings, &mut Velocity, &mut SimPosition, &mut Transform)>,
    pause: Res<Pause>,
    time: Res<Time>,
    speed: Res<Speed>,
    selected_entity: Res<SelectedEntity>,
    mut orbit_offset: ResMut<OrbitOffset>,
    sub_steps: Res<SubSteps>,
    mut nbody_stats: ResMut<NBodyStats>,
    mut diagnostics: Diagnostics
) {
    if pause.0 {
        change_selection_without_update(&mut query, &selected_entity, &mut orbit_offset); //allows switching bodies while paused
        return;
    }
    let delta = time.delta_seconds() as f64;
    let start = Instant::now();
    nbody_stats.steps = 0;
    for _ in 0..sub_steps.0 {
        let start_step = Instant::now();                
        update_acceleration(&mut query, &mut nbody_stats.steps);
        update_velocity_and_positions(&mut query, delta, &speed, &mut nbody_stats.steps, &selected_entity, &mut orbit_offset);
        diagnostics.add_measurement(NBODY_STEP_TIME, || start_step.elapsed().as_nanos() as f64);
    }
    diagnostics.add_measurement(NBODY_TOTAL_TIME, || start.elapsed().as_nanos() as f64);
}

fn update_acceleration(
    query: &mut Query<(Entity, &Mass, &mut Acceleration, &mut OrbitSettings, &mut Velocity, &mut SimPosition, &mut Transform)>,
    steps: &mut i32,
) {
    let mut other_bodies: Vec<(&Mass, Mut<Acceleration>, Mut<SimPosition>)> = Vec::with_capacity(query.iter().count());
    for (_, mass, mut acc, _, _, sim_pos, _) in query.iter_mut() {
        acc.0 = DVec3::ZERO;
        for (other_mass, ref mut other_acc, other_sim_pos) in other_bodies.iter_mut() {
            let distance = other_sim_pos.0 - sim_pos.0;
            let r_sq = distance.length_squared();
            let force_direction = distance.normalize(); // Calculate the direction vector  
            let force_magnitude = G * mass.0 * other_mass.0 / r_sq;
            let force = force_direction * force_magnitude;
            acc.0 += force;
            other_acc.0 -= force;
            *steps += 1;
        }
        other_bodies.push((mass, acc, sim_pos));
    }
}

fn change_selection_without_update(
    query: &mut Query<(Entity, &Mass, &mut Acceleration, &mut OrbitSettings, &mut Velocity, &mut SimPosition, &mut Transform)>,
    selected_entity: &Res<SelectedEntity>,
    orbit_offset: &mut ResMut<OrbitOffset>,
) {
    let offset = match selected_entity.entity { //if orbit_offset.enabled is true, we calculate the new position of the selected entity first and then move it to 0,0,0 and add the actual position to all other bodies
        Some(selected) => {
            if !orbit_offset.enabled {
                DVec3::ZERO
            } else if let Ok((_, _, _, _, _, sim_pos, mut transform)) = query.get_mut(selected) {
                let raw_translation = sim_pos.0 * M_TO_UNIT;
                transform.translation = Vec3::ZERO; //the selected entity will always be at 0,0,0
                -raw_translation 
            } else {
                DVec3::ZERO 
            }
        }
        None => DVec3::ZERO,
    };
    for (entity, _, _, _, _, sim_pos, mut transform) in query.iter_mut() {
        if orbit_offset.enabled {
            if let Some(s_entity) = selected_entity.entity {
                if s_entity == entity {
                    continue;
                }
            }
        }
        let pos_without_offset = sim_pos.0.as_vec3() * M_TO_UNIT as f32;
        transform.translation = pos_without_offset + offset.as_vec3(); //apply offset   
    }
    if orbit_offset.enabled {
        orbit_offset.value = offset.as_vec3();   
    } else {
        orbit_offset.value = Vec3::ZERO
    }
}

fn update_velocity_and_positions(
    query: &mut Query<(Entity, &Mass, &mut Acceleration, &mut OrbitSettings, &mut Velocity, &mut SimPosition, &mut Transform)>,
    delta_time: f64,
    speed: &Res<Speed>,
    steps: &mut i32,
    selected_entity: &Res<SelectedEntity>,
    orbit_offset: &mut ResMut<OrbitOffset>,
) {
    let offset = match selected_entity.entity { //if orbit_offset.enabled is true, we calculate the new position of the selected entity first and then move it to 0,0,0 and add the actual position to all other bodies
        Some(selected) => {
            if !orbit_offset.enabled {
                DVec3::ZERO
            } else if let Ok((_, mass, mut acc, mut orbit_s, mut vel, mut sim_pos, mut transform)) = query.get_mut(selected) {
                orbit_s.force_direction = acc.0.normalize();
                acc.0 /= mass.0; //actually apply the force to the body
                vel.0 += acc.0 * delta_time * speed.0;
                sim_pos.0 += vel.0 * delta_time * speed.0; //this is the same step as below, but we are doing this first for the offset
                let raw_translation = sim_pos.0 * M_TO_UNIT;
                transform.translation = Vec3::ZERO; //the selected entity will always be at 0,0,0
                *steps += 1;
                -raw_translation 
            } else {
                DVec3::ZERO 
            }
        }
        None => DVec3::ZERO,
    };
    for (entity, mass, mut acc, mut orbit_s, mut vel, mut sim_pos, mut transform) in query.iter_mut() {
        if orbit_offset.enabled {
            if let Some(s_entity) = selected_entity.entity {
                if s_entity == entity {
                    continue;
                }
            }
        }
        orbit_s.force_direction = acc.0.normalize();
        acc.0 /= mass.0; //actually apply the force to the body
        vel.0 += acc.0 * delta_time * speed.0;
        *steps += 1;
        sim_pos.0 += vel.0 * delta_time * speed.0;
        let pos_without_offset = sim_pos.0.as_vec3() * M_TO_UNIT as f32;
        transform.translation = pos_without_offset + offset.as_vec3(); //apply offset   
    }
    if orbit_offset.enabled {
        orbit_offset.value = offset.as_vec3();   
    } else {
        orbit_offset.value = Vec3::ZERO
    }
}