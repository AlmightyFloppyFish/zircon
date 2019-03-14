extern crate vulkano;
extern crate vulkano_shaders;

// use crate::render::shader;
use crate::render::vk::{recreate_dimensions_dependent, Vertex};
use std::sync::Arc;

use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, DynamicState},
    descriptor::descriptor_set::PersistentDescriptorSet,
    device,
    format::Format,
    framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass},
    image::{Dimensions, ImmutableImage},
    instance::{Instance, PhysicalDevice},
    pipeline::{vertex::SingleBufferDefinition, viewport::Viewport, GraphicsPipeline},
    sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode},
    swapchain,
    swapchain::{PresentMode, Surface, SurfaceTransform, Swapchain},
    sync::GpuFuture,
};

pub struct VkSession {
    pub instance: Arc<Instance>,
    pub device: Arc<device::Device>,
    pub queue: Arc<device::Queue>,
    // Is it more effective to store dims seperately for draw buffer?
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

pub fn instance() -> Arc<Instance> {
    let extensions = vulkano_win::required_extensions();
    Instance::new(None, &extensions, None).expect("Could not create vulkan instance")
}

pub fn init(instance: Arc<Instance>, surface: &Arc<Surface<winit::Window>>) -> VkSession {
    let physical = PhysicalDevice::enumerate(&instance)
        .next()
        .expect("Device does not support Vulkan");

    let (device, queue) = get_device(&physical);

    let capabilities = surface
        .capabilities(physical)
        .expect("failed to get surface capabilities");

    let dimensions = capabilities.current_extent.unwrap_or([800, 800]);
    let alpha = capabilities
        .supported_composite_alpha
        .iter()
        .next()
        .unwrap();
    let format = capabilities.supported_formats[0].0;

    let (mut swapchain, mut images) = Swapchain::new(
        device.clone(),
        surface.clone(),
        capabilities.min_image_count,
        format,
        dimensions,
        1,
        capabilities.supported_usage_flags,
        &queue,
        SurfaceTransform::Identity,
        alpha,
        PresentMode::Fifo,
        //PresentMode::Immediate,
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

    let (dynamic_state, framebuffers, pipeline) = recreate_dimensions_dependent(
        device.clone(),
        images[0].dimensions(),
        &mut swapchain,
        &mut images,
        &render_pass,
    );

    VkSession {
        instance: instance,
        device: device,
        queue: queue,
        swapchain: swapchain,
        sc_images: images,
        render_pass: render_pass,
        framebuffers: framebuffers,
        pipeline: pipeline,
        dynamic_state: dynamic_state,
    }
}

fn get_device(physical: &PhysicalDevice) -> (Arc<device::Device>, Arc<device::Queue>) {
    let queue_family = physical
        .queue_families()
        .find(|&q| q.supports_graphics())
        .expect("Device does not support vulkan");

    let extensions = device::DeviceExtensions {
        khr_swapchain: true,
        ..device::DeviceExtensions::none()
    };
    let (device, mut queues) = device::Device::new(
        *physical,
        physical.supported_features(),
        &extensions,
        [(queue_family, 0.5)].iter().cloned(),
    )
    .unwrap();

    (device, queues.next().unwrap())
}
