use nalgebra::Vector3;
use rand::Rng;
use std::sync::{Arc, Barrier, RwLock};
use std::thread;
use std::time::{Duration, Instant};

// --- DATA STRUCTURES ---

#[derive(Clone, Copy)]
pub struct Particle {
    pub position: Vector3<f32>,
    pub velocity: Vector3<f32>,
    pub mass: f32,
    pub temperature: f32,
    pub is_sticky: bool,
}

impl Particle {
    pub fn new(pos: Vector3<f32>, vel: Vector3<f32>) -> Self {
        Self {
            position: pos,
            velocity: vel,
            mass: 1.0,        // Baseline mass
            temperature: 1.0, // Start Hot
            is_sticky: false,
        }
    }
}

// --- SIMULATION LOGIC ---

pub fn spawn_particle(initial_speed: f32) -> Particle {
    let mut rng = rand::thread_rng();
    // 0.1m diameter circle at center (0.5, 2.0, 0.5)
    let r = 0.05 * rng.r#gen::<f32>().sqrt();
    let theta = rng.r#gen::<f32>() * 2.0 * std::f32::consts::PI;
    let pos = Vector3::new(0.5 + r * theta.cos(), 2.0, 0.5 + r * theta.sin());

    // Vertical downward cone +/- 30 degrees
    let spread = 30.0f32.to_radians();
    let phi = rng.gen_range(-spread..spread);
    let gamma = rng.gen_range(-spread..spread);
    
    let vel = Vector3::new(phi.sin(), -1.0, gamma.sin()).normalize() * initial_speed;
    Particle::new(pos, vel)
}

fn main() {
    let particle_count = 100_000; // Requirement: 100k+ particles
    let particles = Arc::new(RwLock::new(vec![spawn_particle(2.0); particle_count]));
    let barrier = Arc::new(Barrier::new(6)); // 6 functional threads
    let floor_hits = Arc::new(RwLock::new(0u64));
    
    let dt = 0.016f32; 
    let k_cool = 0.05f32; // Tuneable cooling factor

    println!("Starting High-Performance Shower Simulation...");
    println!("Target: {} particles using 6 threads.", particle_count);

    // --- DYNAMICS THREADS (2) ---
    for t_id in 0..2 {
        let p = Arc::clone(&particles);
        let b = Arc::clone(&barrier);
        let fh = Arc::clone(&floor_hits);
        thread::spawn(move || loop {
            {
                let mut data = p.write().unwrap();
                let len = data.len();
                let start = t_id * (len / 2);
                let end = if t_id == 1 { len } else { (t_id + 1) * (len / 2) };

                for i in start..end {
                    let mut part = data[i]; 
                    part.velocity.y -= 9.81 * dt; 
                    part.position += part.velocity * dt; 

                    if part.position.y <= 0.0 {
                        part = spawn_particle(2.0); 
                        let mut count = fh.write().unwrap();
                        *count += 1; 
                    } else if part.position.x <= 0.0 || part.position.x >= 1.0 || 
                              part.position.z <= 0.0 || part.position.z >= 1.0 {
                        part.is_sticky = true;
                        part.velocity.x = 0.0; // Sticky wall: horizontal velocity = 0
                        part.velocity.z = 0.0;
                    }
                    data[i] = part;
                }
            } // Lock is dropped here so other threads can proceed
            b.wait(); b.wait(); b.wait();
        });
    }

    // --- COLLISION THREADS (2) ---
    for _t_id in 0..2 {
        let p = Arc::clone(&particles);
        let b = Arc::clone(&barrier);
        thread::spawn(move || loop {
            b.wait(); 
            {
                let mut _data = p.write().unwrap();
                // Collision logic/merging would happen here
            }
            b.wait(); b.wait();
        });
    }

    // --- THERMODYNAMICS THREADS (2) ---
    for t_id in 0..2 {
        let p = Arc::clone(&particles);
        let b = Arc::clone(&barrier);
        thread::spawn(move || loop {
            b.wait(); b.wait();
            {
                let mut data = p.write().unwrap();
                let len = data.len();
                let start = t_id * (len / 2);
                let end = if t_id == 1 { len } else { (t_id + 1) * (len / 2) };

                for i in start..end {
                    let cooling = (dt * k_cool) / data[i].mass;
                    data[i].temperature = (data[i].temperature - cooling).max(0.0);
                }
            }
            b.wait();
        });
    }

    // --- MONITORING LOOP (Main Thread) ---
    let start_time = Instant::now();
    loop {
        thread::sleep(Duration::from_millis(1000));
        // We use a read lock here so we don't block the writers too long
        if let Ok(data) = particles.read() {
            let hits = floor_hits.read().unwrap();
            println!(
                "[{:?}] Floor Hits: {} | Particle[0] Temp: {:.4} | Pos: {:.2?}", 
                start_time.elapsed().as_secs(), 
                *hits, 
                data[0].temperature,
                data[0].position
            );
        }
    }
}