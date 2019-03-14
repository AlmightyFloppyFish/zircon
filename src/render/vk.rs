use crate::entity::Rect;
use crate::render::shader;
use crate::render::vkinit::VkSession;

use std::slice::Iter;
use std::sync::{mpsc::Receiver, Arc, Mutex};

use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, DynamicState},
    descriptor::descriptor_set::{DescriptorSet, PersistentDescriptorSet},
    device::{Device, Queue},
    framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass},
    image::{Dimensions, ImmutableImage, SwapchainImage},
    pipeline::{vertex::SingleBufferDefinition, viewport::Viewport, GraphicsPipeline},
    sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode},
    swapchain,
    swapchain::Swapchain,
    sync::GpuFuture,
};

pub type DrawBuffer = Arc<Mutex<Vec<(Arc<DescriptorSet + Send + Sync>, Arc<Rect>, (u32, u32))>>>;
pub type WaitBuffer = Arc<Mutex<Vec<Box<vulkano::sync::GpuFuture + Send + Sync>>>>;

#[derive(Debug, Clone)]
pub struct Vertex {
    position: [f32; 2],
    window_dimensions: [f32; 2],
    image_dimensions: [f32; 2],
}
vulkano::impl_vertex!(Vertex, position, window_dimensions, image_dimensions);

impl Vertex {
    pub fn from(r: Rect, window: [u32; 2], image: (u32, u32)) -> [Vertex; 4] {
        //let (x, y) = (r.position_x * 2.0 - 1.0, r.position_y * 2.0 - 1.0);
        let (x, y) = (r.position_x, r.position_y);

        let (i_x, i_y) = (image.0 as f32, image.1 as f32);

        [
            Vertex {
                // Top-Left
                position: [x, y],
                image_dimensions: [i_x, i_y],
                window_dimensions: [window[0] as f32, window[1] as f32],
            },
            Vertex {
                // Bottom-Left
                position: [x, y + (r.height * 2.0 / 1.0)],
                image_dimensions: [i_x, i_y],
                window_dimensions: [window[0] as f32, window[1] as f32],
            },
            Vertex {
                // Top-Right
                position: [x + (r.width * 2.0 / 1.0), y],
                image_dimensions: [i_x, i_y],
                window_dimensions: [window[0] as f32, window[1] as f32],
            },
            Vertex {
                // Bottom-Right
                position: [x + (r.width * 2.0 / 1.0), y + (r.height * 2.0 / 1.0)],
                image_dimensions: [i_x, i_y],
                window_dimensions: [window[0] as f32, window[1] as f32],
            },
        ]
    }
}

impl VkSession {
    pub fn update_swapchain(&mut self, dimensions: [u32; 2]) {
        let (dynamic_state, framebuffers, pipeline) = recreate_dimensions_dependent(
            self.device.clone(),
            dimensions,
            &mut self.swapchain,
            &mut self.sc_images,
            &mut self.render_pass,
        );
        self.framebuffers = framebuffers;
        self.dynamic_state = dynamic_state;
        self.pipeline = pipeline;
    }

    pub fn present(
        &self,
        mut previous_frame_end: Box<GpuFuture + Sync + Send>,
        draw_buffer: DrawBuffer,
        wait_buffer: WaitBuffer,
    ) -> (Box<GpuFuture + Sync + Send>, bool) {
        let (buffer_num, gpu_fut) =
            match swapchain::acquire_next_image(self.swapchain.clone(), None) {
                Err(_) => {
                    return (previous_frame_end, true);
                }
                Ok((b, f)) => (b, f),
            };
        previous_frame_end.cleanup_finished();

        let mut command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(
            self.device.clone(),
            self.queue.family(),
        )
        .unwrap()
        .begin_render_pass(
            self.framebuffers[buffer_num].clone(),
            false,
            vec![[0.0, 0.0, 0.0, 1.0].into()],
        )
        .unwrap();

        let draws = draw_buffer.lock().unwrap();

        for i in 0..draws.len() {
            let vertex_buffer = CpuAccessibleBuffer::<[Vertex]>::from_iter(
                self.device.clone(),
                BufferUsage::all(),
                Vertex::from(
                    (*draws[i].1).clone(),
                    self.swapchain.dimensions(),
                    draws[i].2.clone(), // Image dims
                )
                .iter()
                .cloned(),
            )
            .unwrap();

            command_buffer = command_buffer
                .draw(
                    self.pipeline.clone(),
                    &self.dynamic_state,
                    vertex_buffer,
                    draws[i].0.clone(),
                    (),
                )
                .unwrap();
        }
        let mut awaits = wait_buffer.lock().unwrap();

        previous_frame_end = Box::new(previous_frame_end.join(gpu_fut));
        for f in awaits.drain(0..) {
            previous_frame_end = Box::new(previous_frame_end.join(f));
        }

        let cb = command_buffer
            .end_render_pass()
            .map_err(|e| eprintln!("\n\n{:?}\n\n", e))
            .unwrap()
            .build()
            .map_err(|e| eprintln!("\n\n{:?}\n\n", e))
            .unwrap();
        let f = match previous_frame_end
            .then_execute(self.queue.clone(), cb)
            .map_err(|e| eprintln!("\n\n\n\nFIRST: {:?}\n\n\n\n", e))
            .unwrap()
            .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), buffer_num)
            .then_signal_fence_and_flush()
        {
            Err(e) => {
                eprintln!("Skipping frame because {:?}", e);
                return (
                    Box::new(vulkano::sync::now(self.device.clone()))
                        as Box<GpuFuture + Send + Sync>,
                    true,
                );
            }
            Ok(cb) => cb,
        };

        return (Box::new(f) as Box<GpuFuture + Sync + Send>, false);
    }
}

pub fn spawn_render_thread(
    img_recv: Arc<Mutex<Receiver<(Vec<u8>, (u32, u32), Rect)>>>,
    queue: Arc<Queue>,
    device: Arc<Device>,
    pipeline: Arc<
        GraphicsPipeline<
            SingleBufferDefinition<Vertex>,
            Box<vulkano::descriptor::PipelineLayoutAbstract + Send + Sync>,
            Arc<RenderPassAbstract + Send + Sync>,
        >,
    >,
    draw_buffer: DrawBuffer,
    wait_buffer: WaitBuffer,
) {
    std::thread::spawn(move || {
        let sampler = default_sampler(device.clone());
        loop {
            let (img_data, dimensions, rect) = img_recv.lock().unwrap().recv().unwrap();

            println!(
                "Saving image as ImmutableImage buffer with res {}x{}",
                dimensions.0, dimensions.1
            );
            let ft = std::time::SystemTime::now();
            let (texture, future) = ImmutableImage::from_iter(
                img_data.iter().cloned(),
                Dimensions::Dim2d {
                    width: dimensions.0,
                    height: dimensions.1,
                },
                vulkano::format::Format::R8G8B8A8Srgb,
                queue.clone(),
            )
            .unwrap();
            println!("{:?}", ft.elapsed());

            let set = Arc::new(
                PersistentDescriptorSet::start(pipeline.clone(), 0)
                    .add_sampled_image(texture, sampler.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            );

            draw_buffer
                .lock()
                .unwrap()
                .push((set, Arc::new(rect), dimensions));
            wait_buffer.lock().unwrap().push(Box::new(future));
        }
    });
}

fn default_sampler(device: Arc<Device>) -> Arc<Sampler> {
    Sampler::new(
        device,
        Filter::Linear,
        Filter::Linear,
        MipmapMode::Nearest,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::Repeat,
        0.0,
        1.0,
        0.0,
        0.0,
    )
    .unwrap()
}

pub fn recreate_dimensions_dependent(
    device: Arc<Device>,
    dimensions: [u32; 2],
    swapchain: &mut Arc<Swapchain<winit::Window>>,
    images: &mut Vec<Arc<SwapchainImage<winit::Window>>>,
    render_pass: &std::sync::Arc<dyn RenderPassAbstract + std::marker::Send + std::marker::Sync>,
) -> (
    DynamicState,
    Vec<Arc<FramebufferAbstract + Send + Sync>>,
    Arc<
        GraphicsPipeline<
            SingleBufferDefinition<Vertex>,
            Box<vulkano::descriptor::PipelineLayoutAbstract + Send + Sync>,
            Arc<RenderPassAbstract + Send + Sync>,
        >,
    >,
) {
    let new = swapchain.recreate_with_dimension(dimensions).unwrap();
    *swapchain = new.0;
    *images = new.1;

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0..1.0,
    };
    let dynamic_state = DynamicState {
        line_width: None,
        viewports: Some(vec![viewport]),
        scissors: None,
    };

    let framebuffers = {
        images
            .iter()
            .map(|image| {
                Arc::new(
                    Framebuffer::start(render_pass.clone())
                        .add(image.clone())
                        .unwrap()
                        .build()
                        .unwrap(),
                ) as Arc<FramebufferAbstract + Send + Sync>
            })
            .collect::<Vec<_>>()
    };

    let vs = shader::vs::Shader::load(device.clone()).unwrap();
    let fs = shader::fs::Shader::load(device.clone()).unwrap();

    let pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer::<Vertex>()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_strip()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .blend_alpha_blending()
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap(),
    );

    println!("{:?}", &dimensions);
    (dynamic_state, framebuffers, pipeline)
}
