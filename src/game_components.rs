#[derive(Copy, Clone, Debug)]
pub struct Float3(pub f32, pub f32, pub f32);
#[derive(Copy, Clone, Debug)]
pub struct _Float4(pub f32, pub f32, pub f32, pub f32);

#[derive(Copy, Clone, Debug)]
pub struct Entity {
    pub pos: Float3,
    pub rot: Float3,
    pub scl: Float3,
    pub spd: f32,
    pub max_spd: f32,
}
