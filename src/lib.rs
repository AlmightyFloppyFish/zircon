pub mod entity;
mod framecounter;
mod render;
extern crate image;
extern crate winit;

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use image::*;

use render::vk;

use std::collections::HashMap;

use vulkano::format::Format;
use vulkano::image::{Dimensions, ImmutableImage};
use vulkano::sync::GpuFuture;
use vulkano_win::VkSurfaceBuild;

use winit::Event;
use winit::WindowEvent;

struct Settings {
    framelimit: u16,
}

pub struct Game {
    settings: Settings,
    pub active_textures: HashMap<String, bool>,
    pub textures: Vec<entity::Texture>,
}

impl Game {
    pub fn init() -> Self {
        Game {
            settings: Settings { framelimit: 144 },
            active_textures: HashMap::new(),
            textures: Vec::new(),
        }
    }

    pub fn connect(
        &mut self,
        entity: Box<entity::Entity + Send + Sync>,
        rect: entity::Rect,
        img_path: &str,
    ) -> Result<(), String> {
        let img = match image::open(img_path) {
            Err(e) => return Err(format!("Could not open {}: {}", img_path, e)),
            Ok(i) => i,
        };
        let (w, h) = (img.width(), img.height());

        let raw = img.to_rgba().into_raw();
        self.textures.push(entity::Texture {
            rect: rect,
            entity: entity,
            sprite: (raw, (w, h)),
        });
        Ok(())
    }

    // pub fn deactivate(&mut self, e: &entity::Entity) {}

    pub fn run(mut self) {
        let vk_instance = render::vkinit::instance();

        // Winit
        let mut events_loop = winit::EventsLoop::new();
        let monitor = events_loop.get_primary_monitor();
        let surface = winit::WindowBuilder::new()
            .with_resizable(false)
            //.with_fullscreen(Some(monitor))
            .build_vk_surface(&events_loop, vk_instance.clone())
            .unwrap();

        // Vulkan
        let mut vk = render::vkinit::init(vk_instance, &surface);;

        // Prepare threadding
        // Game session
        let data_event = Arc::new(Mutex::new(self));
        let data_user = data_event.clone();
        let data_backend = data_event.clone();
        // Draw buffers
        let draw_buffer: render::vk::DrawBuffer = Arc::new(Mutex::new(Vec::new()));
        let wait_buffer: render::vk::WaitBuffer = Arc::new(Mutex::new(Vec::new()));
        // Draw Selector
        let (img_send, img_recv_raw) = mpsc::channel();
        let img_recv = Arc::new(Mutex::new(img_recv_raw));

        for _ in 0..4 {
            vk::spawn_render_thread(
                img_recv.clone(),
                vk.queue.clone(),
                vk.device.clone(),
                vk.pipeline.clone(),
                draw_buffer.clone(),
                wait_buffer.clone(),
            );
        }

        // Vk safety
        let mut vk_previous_frame_end =
            Box::new(vulkano::sync::now(vk.device.clone())) as Box<GpuFuture + Send + Sync>;
        let vk = Arc::new(Mutex::new(vk));
        let vk_event = vk.clone();

        // User loop
        thread::spawn(move || loop {
            // User defined code-per-entity gets run
            thread::sleep(Duration::from_millis((1000 / 60) as u64));
            let mut data = data_user.lock().unwrap();
            for texture in &mut data.textures {
                texture.entity.update(&mut texture.rect);
            }
        });

        let mut fps = framecounter::FPSCounter::new();

        // Backend loop
        thread::spawn(move || {
            let data = data_backend.lock().unwrap();

            for texture in &data.textures {
                img_send
                    .send((
                        texture.sprite.0.clone(),
                        ((texture.sprite.1).0, (texture.sprite.1).1),
                        texture.rect.clone(),
                    ))
                    .unwrap();
            }
            drop(data);
            let window = surface.window();

            loop {
                let data = data_backend.lock().unwrap();
                let mut db = draw_buffer.lock().unwrap();
                if data.textures.len() != db.len() {
                    // Oh no, all textures haven't been loaded yet
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                for i in 0..data.textures.len() {
                    if data.textures[i].rect != *db[i].1 {
                        db[i].1 = Arc::new(data.textures[i].rect.clone());
                    }
                }
                drop(data);
                drop(db);

                let mut vk = vk.lock().unwrap();
                let res = vk.present(
                    vk_previous_frame_end,
                    draw_buffer.clone(),
                    wait_buffer.clone(),
                );
                fps.tick_and_display();
                vk_previous_frame_end = res.0;
                if res.1 {
                    let dims: (u32, u32) = window
                        .get_inner_size()
                        .unwrap()
                        .to_physical(window.get_hidpi_factor())
                        .into();
                    dbg!(dims);
                    vk.update_swapchain([dims.0, dims.1]);
                }
            }
        });

        // Event loop |-> blocking
        events_loop.run_forever(|event| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => winit::ControlFlow::Break,
            _ => winit::ControlFlow::Continue,
        });
    }
}
