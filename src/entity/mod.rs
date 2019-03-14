use crate::Game;

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    pub width: f32,
    pub height: f32,
    pub position_x: f32,
    pub position_y: f32,
}

impl Rect {
    pub fn new(w: f32, h: f32, x: f32, y: f32) -> Self {
        Rect {
            width: w,
            height: h,
            position_x: x,
            position_y: y,
        }
    }
}

pub trait Entity {
    fn init(&mut self);
    fn update(&mut self, rect: &mut Rect);
    // fn events<F>(HashMap<Event, F>) {}
}

pub struct Texture {
    pub rect: Rect,
    pub entity: Box<Entity + Send + Sync>,
    pub sprite: (Vec<u8>, (u32, u32)),
}
