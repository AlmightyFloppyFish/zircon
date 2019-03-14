extern crate vulkano;
extern crate vulkano_shaders;

extern crate image;

use std::error::Error;
use std::sync::Arc;

mod shader;

use crate::entity::Rect;

use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, DynamicState},
    descriptor::descriptor_set::PersistentDescriptorSet,
    device::{Device, Queue},
    format::Format,
    framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass},
    image::{Dimensions, ImmutableImage},
    instance::{Instance, PhysicalDevice},
    pipeline::{vertex::SingleBufferDefinition, viewport::Viewport, GraphicsPipeline},
    sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode},
    swapchain,
    swapchain::{PresentMode, SurfaceTransform, Swapchain},
    sync::GpuFuture,
};

pub struct VkSession {
    pub instance: Arc<Instance>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain<winit::Window>>,
    pub sc_images: Vec<Arc<vulkano::image::SwapchainImage<winit::Window>>>,
    pub render_pass: Arc<RenderPassAbstract + Send + Sync>,
    pub framebuffers: Vec<Arc<FramebufferAbstract + Send + Sync>>,
    pub dynamic_state: DynamicState,
    pub pipeline: Arc<
        GraphicsPipeline<
            SingleBufferDefinition<Vertex>,
            Box<vulkano::descriptor::PipelineLayoutAbstract + Send + Sync>,
            Arc<RenderPassAbstract + Send + Sync>,
        >,
    >,
}

pub fn vk_new_instance() -> Arc<Instance> {
    let extensions = vulkano_win::required_extensions();
    Instance::new(None, &extensions, None).expect("Could not create vulkan instance")
}

#[derive(Debug, Clone)]
pub struct Vertex {
    position: [f32; 2],
}
vulkano::impl_vertex!(Vertex, position);

impl Vertex {
    pub fn from(r: Rect) -> [Vertex; 4] {
        let (x, y) = (r.position_x * 2.0 - 1.0, r.position_y * 2.0 - 1.0);
        [
            Vertex {
                // Top-Left
                position: [x, y],
            },
            Vertex {
                // Bottom-Left
                position: [x, y + (r.height * 2.0 / 1.0)],
            },
            Vertex {
                // Top-Right
                position: [x + (r.width * 2.0 / 1.0), y],
            },
            Vertex {
                // Bottom-Right
                position: [x + (r.width * 2.0 / 1.0), y + (r.height * 2.0 / 1.0)],
            },
        ]
    }
}

impl VkSession {
    pub fn render_image(&self, img_data: (&std::slice::Iter<u8>, (u32, u32)), r: Rect) {
        // let image: image::RgbaImage = image::open(img).unwrap().to_rgba();
        //

        let (buffer_num, gpu_has_img) =
            swapchain::acquire_next_image(self.swapchain.clone(), None).unwrap();

        /*let dynamic_state = DynamicState {
            line_width: None,
            viewports: None,
            scissors: None,
        };*/

        // TODO: This from params
        let vertex_buffer = CpuAccessibleBuffer::<[Vertex]>::from_iter(
            self.device.clone(),
            BufferUsage::all(),
            Vertex::from(r).iter().cloned(),
        )
        .unwrap();

        // THIS TAKES TIME.
        // I should only do this once, and only update it if the user calls a change method
        let (texture, texture_is_loaded_future) = {
            ImmutableImage::from_iter(
                img_data.0.cloned(),
                Dimensions::Dim2d {
                    width: (img_data.1).0,
                    height: (img_data.1).1,
                },
                Format::R8G8B8A8Srgb,
                self.queue.clone(),
            )
            .unwrap()
        };

        let sampler = Sampler::new(
            self.device.clone(),
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
        .unwrap();

        let set = Arc::new(
            PersistentDescriptorSet::start(self.pipeline.clone(), 0)
                .add_sampled_image(texture.clone(), sampler.clone())
                .unwrap()
                .build()
                .unwrap(),
        );

        let command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(
            self.device.clone(),
            self.queue.family(),
        )
        .unwrap()
        .begin_render_pass(
            self.framebuffers[buffer_num].clone(),
            false,
            vec![[0.0, 0.0, 0.0, 1.0].into()],
        )
        .unwrap()
        .draw(
            self.pipeline.clone(),
            &self.dynamic_state,
            vertex_buffer.clone(),
            set.clone(),
            (),
        )
        .unwrap()
        .end_render_pass()
        .unwrap()
        .build()
        .unwrap();

        // Return all these futures and handle on level below, this would make it concurrent

        let mut future = Box::new(texture_is_loaded_future) as Box<GpuFuture>;
        future.cleanup_finished();

        let future = future
            .join(gpu_has_img)
            .then_execute(self.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), buffer_num)
            .then_signal_fence_and_flush();
        // TODO: I probably want to do something with future
    }

    // pub fn refresh() {}
}

pub fn init(
    instance: Arc<Instance>,
    surface: &Arc<vulkano::swapchain::Surface<winit::Window>>,
) -> Result<VkSession, Box<Error>> {
    let physical = PhysicalDevice::enumerate(&instance)
        .next()
        .expect("Device does not support vulkan");

    let queue_family = physical
        .queue_families()
        .find(|&q| q.supports_graphics())
        .expect("Device does not support vulkan");

    let (device, mut queues) = {
        let device_ext = vulkano::device::DeviceExtensions {
            khr_swapchain: true,
            ..vulkano::device::DeviceExtensions::none()
        };

        Device::new(
            physical,
            physical.supported_features(),
            &device_ext,
            [(queue_family, 0.5)].iter().cloned(),
        )?
    };

    let capibilities = surface
        .capabilities(physical)
        .expect("failed to get surface capibilities");

    let dimensions = capibilities.current_extent.unwrap_or([1280, 1024]);
    let alpha = capibilities
        .supported_composite_alpha
        .iter()
        .next()
        .unwrap();
    let format = capibilities.supported_formats[0].0;

    let queue = queues.next().unwrap();

    let (swapchain, images) = Swapchain::new(
        device.clone(),
        surface.clone(),
        capibilities.min_image_count,
        format,
        dimensions,
        1,
        capibilities.supported_usage_flags,
        &queue,
        SurfaceTransform::Identity,
        alpha,
        PresentMode::Fifo,
        true,
        None,
    )
    .expect("Failed to create swapchain");

    let render_pass = Arc::new(
        vulkano::single_pass_renderpass!(device.clone(),
        attachments: {
            color: {
                load: Clear,
                store: Store,
                format: swapchain.format(),
                samples: 1,
            }
        },
        pass: {
            color: [color],
            depth_stencil: {}
        }
        )
        .unwrap(),
    ) as Arc<RenderPassAbstract + Send + Sync>;

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

    // I need to rerun this at each resize change...
    let dimensions = images[0].dimensions();
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

    Ok(VkSession {
        instance: instance,
        device: device,
        queue: queue,
        swapchain: swapchain,
        sc_images: images,
        render_pass: render_pass,
        framebuffers: framebuffers,
        pipeline: pipeline,
        dynamic_state: dynamic_state,
    })
}
