extern crate vulkano_shaders;

pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
#version 450
layout(location = 0) in vec2 position;
layout(location = 1) in vec2 window_dimensions;
layout(location = 2) in vec2 image_dimensions;

layout(location = 0) out vec2 tex_coords;

void main() {

    vec2 scale;
    scale.x = image_dimensions.x / window_dimensions.x;
    scale.y = image_dimensions.y / window_dimensions.y;

    gl_Position = vec4(position, 0.0, 1.0);
    tex_coords = (position) + vec2(scale.x, scale.y);
}"
    }
}

pub mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
#version 450
layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D tex;
void main() {
    f_color = texture(tex, tex_coords);
}"
    }
}
