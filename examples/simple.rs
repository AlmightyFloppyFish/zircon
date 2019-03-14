extern crate zircon;

use zircon::entity::{Entity, Rect};
use zircon::Game;

struct Person {
    name: String,
    status: String,
    right: bool,
}

impl Entity for Person {
    fn init(&mut self) {
        // Happens each time this Person is spawned
    }
    fn update(&mut self, rect: &mut Rect) {
        /*
        if self.right {
            rect.position_x += 0.01;
        } else {
            rect.position_x -= 0.01;
        }
        if rect.position_x <= 0.1 && rect.position_x >= 0.05 {
            self.right = true;
        } else if rect.position_x >= 0.9 && rect.position_x <= 0.95 {
            self.right = false;
        };
        */
    }
}

fn main() {
    let mut game = zircon::Game::init();

    let mut simon = Person {
        name: String::from("simon"),
        status: String::from("chilling"),
        right: true,
    };
    game.connect(
        Box::new(simon),
        Rect::new(0.5, 0.5, -1.0, -1.0),
        "nature.png",
    )
    .expect("Could not load nature.png");

    game.run();
}
