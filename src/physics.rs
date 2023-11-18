use std::time::{Instant, Duration};

use bevy::app::{App, Plugin, Update};
use bevy::math::{DVec3, Vec3};
use bevy::prelude::{Entity, in_state, IntoSystemConfigs, Mut, Query, Res, ResMut, Resource, Time, Transform};

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
            .init_resource::<ApproximationSettings>()
            .init_resource::<SubSteps>()
            .init_resource::<NBodyStats>()
            .register_type::<Velocity>()
            .register_type::<Acceleration>()
            .register_type::<Mass>()
            .register_type::<SimPosition>()
            .register_type::<OrbitSettings>()
            .add_systems(Update, (apply_physics).run_if(in_state(SimState::Simulation)));
    }
}

#[derive(Resource, Default)]
pub struct Pause(pub bool);

#[derive(Resource)]
pub struct ApproximationSettings {
    pub leap_frog: bool,
    pub revo_approximation: bool,
}

impl Default for ApproximationSettings {
    fn default() -> Self {
        Self {
            leap_frog: false,
            revo_approximation: true,
        }
    }
}

#[derive(Resource)]
pub struct SubSteps(pub i32);

#[derive(Resource, Default)]
pub struct NBodyStats {
    
    pub time: Duration,
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

pub fn apply_physics(
    mut query: Query<(Entity, &Mass, &mut Acceleration, &mut Velocity, &mut SimPosition, &mut Transform)>,
    pause: Res<Pause>,
    time: Res<Time>,
    speed: Res<Speed>,
    selected_entity: Res<SelectedEntity>,
    mut orbit_offset: ResMut<OrbitOffset>,
    sub_steps: Res<SubSteps>,
    mut nbody_stats: ResMut<NBodyStats>,
    approximation_settings: Res<ApproximationSettings>,
) {
    if pause.0 {
        return;
    }
    let delta = time.delta_seconds() as f64;
    let start = Instant::now();
    nbody_stats.steps = 0;
    if approximation_settings.revo_approximation {
        for _ in 0..sub_steps.0 {
            update_acceleration(&mut query, &mut nbody_stats.steps);
            update_velocity_and_positions(&mut query, delta, &speed, &mut nbody_stats.steps, &selected_entity, &mut orbit_offset, approximation_settings.leap_frog);
        }
    }
    else {
        for _ in 0..sub_steps.0 {
            update_acceleration(&mut query, &mut nbody_stats.steps);
            update_velocity_and_positions(&mut query, delta, &speed, &mut nbody_stats.steps, &selected_entity, &mut orbit_offset, approximation_settings.leap_frog);
        }
    }
    nbody_stats.time = start.elapsed();
}

fn update_acceleration(
    query: &mut Query<(Entity, &Mass, &mut Acceleration, &mut Velocity, &mut SimPosition, &mut Transform)>,
    steps: &mut i32,
) {
    let mut other_bodies: Vec<(&Mass, Mut<Acceleration>, Mut<SimPosition>)> = Vec::new();
    for (_, mass, mut acc, _, sim_pos, _) in query.iter_mut() {
        acc.0 = DVec3::ZERO;
        for (other_mass, ref mut other_acc, other_sim_pos) in other_bodies.iter_mut() {
            let r_sq = (sim_pos.0 - other_sim_pos.0).length_squared();
            let force_direction = DVec3::from((other_sim_pos.0 - sim_pos.0).normalize()); // Calculate the direction vector  
            
            let force_magnitude = G * mass.0 * other_mass.0 / r_sq;
            let force = force_direction * force_magnitude;
            acc.0 += force;
            other_acc.0 -= force;
            *steps += 1;
        }
        other_bodies.push((mass, acc, sim_pos));
    }
}

fn update_velocity_and_positions(
    query: &mut Query<(Entity, &Mass, &mut Acceleration, &mut Velocity, &mut SimPosition, &mut Transform)>,
    delta_time: f64,
    speed: &Res<Speed>,
    steps: &mut i32,
    selected_entity: &Res<SelectedEntity>,
    orbit_offset: &mut ResMut<OrbitOffset>,
    leap_frog: bool,
) {
    for (_, mass, mut acc, mut vel, _, _) in query.iter_mut() {
        acc.0 /= mass.0; //actually apply the force to the body
        if leap_frog {
            vel.0 += acc.0 * delta_time * speed.0 * 0.5; //apply 0.5 of the acceleration
        }
        else {
            vel.0 += acc.0 * delta_time * speed.0 //apply normal acceleration
        }
        *steps += 1;
    }
    let offset = match selected_entity.entity {
        Some(selected) => {
            if let Ok((_, _, acc, mut vel, mut sim_pos, mut transform)) = query.get_mut(selected) {
                sim_pos.0 += vel.0 * delta_time * speed.0; //this is the same step as below, but we are doing this first for the offset
                let raw_translation = sim_pos.0 * M_TO_UNIT;
                transform.translation = Vec3::ZERO; //the selected entity will always be at 0,0,0
                *steps += 1;
                if leap_frog {
                    vel.0 += acc.0 * delta_time * speed.0 * 0.5; //apply 0.5 of the acceleration a second time (the selected entity will be ignored in the loop below)
                }
                -raw_translation 
            } else {
                DVec3::ZERO 
            }
        }
        None => DVec3::ZERO,
    };
    for (entity, _, acc, mut vel, mut sim_pos, mut transform) in query.iter_mut() {
        if let Some(s_entity) = selected_entity.entity {
            if s_entity == entity {
                continue;
            }
        }
        *steps += 1;
        sim_pos.0 += vel.0 * delta_time * speed.0;
        let pos_without_offset = sim_pos.0.as_vec3() * M_TO_UNIT as f32;
        transform.translation = pos_without_offset + offset.as_vec3(); //apply offset
        if leap_frog {
            vel.0 += acc.0 * delta_time * speed.0 * 0.5; //apply 0.5 of the acceleration a second time
        }
    }
    orbit_offset.0 = offset.as_vec3();
}