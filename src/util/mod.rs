#[macro_use]
mod macros;
pub use macros::hashmap;

// pub mod registry;

use std::time::{Duration, SystemTime};

pub mod iter {
    use bevy::math::IVec3;

    // [min to max] yzx order
    pub fn iter_aabb(nxz: i32, ny: i32, mut func: impl FnMut(IVec3)) {
        for ly in -ny..=ny {
            for lz in -nxz..=nxz {
                for lx in -nxz..=nxz {
                    func(IVec3::new(lx, ly, lz));
                }
            }
        }
    }

    pub fn iter_xzy(n: i32, mut func: impl FnMut(IVec3)) {
        assert!(n > 0);
        for ly in 0..n {
            for lz in 0..n {
                for lx in 0..n {
                    func(IVec3::new(lx, ly, lz));
                }
            }
        }
    }
}


use bevy::prelude::*;

pub fn hash(i: i32) -> f32 {
    let i = (i << 13) ^ i;
    // (((i * i * 15731 + 789221) * i + 1376312589) as u32 & 0xffffffffu32) as f32 / 0xffffffffu32 as f32
    // wrapping_mul: avoid overflow
    let i = i.wrapping_mul(i).wrapping_mul(15731).wrapping_add(789221).wrapping_mul(i).wrapping_add(1376312589);
    (i as u32 & 0xffffffffu32) as f32 / 0xffffffffu32 as f32
}
pub fn hash3(v: IVec3) -> Vec3 {
    Vec3::new(hash(v.x), hash(v.y), hash(v.z))
}




pub fn current_timestamp() -> Duration {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap()
}

pub fn current_timestamp_millis() -> u64 {
    current_timestamp().as_millis() as u64
}

pub trait TimeIntervals {
    fn intervals(&self, interval: f32) -> usize;

    fn at_interval(&self, interval: f32) -> bool {
        self.intervals(interval) != 0
    }

    fn _intervals(t: f32, dt: f32, u: f32) -> usize {
        ((t / u).floor() - ((t - dt) / u).floor()) as usize
    }
}
impl TimeIntervals for bevy::time::Time {
    fn intervals(&self, u: f32) -> usize {
        Self::_intervals(self.elapsed_seconds(), self.delta_seconds(), u)
    }
}


use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

pub fn hashcode<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}



#[derive(Default)]
pub struct SmoothValue {
    pub target: f32,
    pub current: f32,
}

impl SmoothValue {
    pub fn update(&mut self, dt: f32) {
        self.current += dt * (self.target - self.current);
    }
}