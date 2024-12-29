#[derive(Copy, Clone)]
pub struct Float3(pub f32, pub f32, pub f32);
#[derive(Copy, Clone)]
pub struct Float4(pub f32, pub f32, pub f32, pub f32);

#[derive(Copy, Clone)]
pub struct Entity {
    pub pos: Float3,
    pub rot: Float3,
    pub scl: Float3,
    pub spd: Float3,
    pub max_spd: f32,
}
