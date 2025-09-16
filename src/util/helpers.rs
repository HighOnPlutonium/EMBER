use std::error::Error;
use std::ptr;
use ash::{khr, vk, Device};
use shaderc::CompilationArtifact;
use winit::dpi::PhysicalSize;
use winit::window::Window;
use crate::MAX_FRAMES_IN_FLIGHT;
use crate::util::per_window::{PerWindow, SYN};

//the bare minimum
fn load_shaders(source: &str, kind: shaderc::ShaderKind) -> CompilationArtifact {
    let compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    //specify the entry point - here, it's "main"
    options.add_macro_definition("EP", Some("main"));
    compiler.compile_into_spirv(
        source, kind,
        //those two strings are really just there for (possible) error messages. they don't need to be correct at all.
        "shader.glsl", "main", Some(&options)).unwrap()
}




pub(crate) unsafe fn create_swapchain(
    window: &Window,
    surface: vk::SurfaceKHR,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    ext_surface: &khr::surface::Instance,
    ext_swapchain: &khr::swapchain::Device
) -> Result<(vk::SwapchainKHR,vk::Format,vk::Extent2D,Vec<SYN>),Box<dyn Error>> { //currently we just propagate possible issues to the caller. which kinda sucks.
    //if we want to do anything fun we'll need a swapchain - and that's a per-surface thingy
    // todo! actually use all this information, and decide on proper swapchain settings based on them
    let capabilities = ext_surface.get_physical_device_surface_capabilities(physical_device, surface)?;
    let formats = ext_surface.get_physical_device_surface_formats(physical_device, surface)?;
    #[allow(unused)]
    let present_modes = ext_surface.get_physical_device_surface_present_modes(physical_device, surface)?;
    #[allow(unused)]
    let (formats, color_spaces) = formats.iter().map(|format| (format.format, format.color_space)).collect::<(Vec<vk::Format>, Vec<vk::ColorSpaceKHR>)>();

    //in case neither 32bit BGRA SRGB or 32bit RGBA SRGB are available, a default value.
    let mut format = *formats.first().unwrap();
    //ain't fuckin with any of the other color spaces, nor dealing with their availability for now. honestly go find a device other than a washing mashien or something that deosn't support SRGB color spaces
    let color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR;
    //SRGB is common and good. B8G8R8 format is also shockingly common in displays?
    if formats.contains(&vk::Format::B8G8R8A8_SRGB) { format = vk::Format::B8G8R8A8_SRGB } else if formats.contains(&vk::Format::R8G8B8A8_SRGB) { format = vk::Format::R8G8B8A8_SRGB }

    let extent = {
        let PhysicalSize { width, height } = window.inner_size();
        vk::Extent2D::default().width(width).height(height)
    };

    let swapchain_create_info = vk::SwapchainCreateInfoKHR {
        flags: vk::SwapchainCreateFlagsKHR::default(),
        surface,
        min_image_count: capabilities.min_image_count,
        image_format: format,
        image_color_space: color_space,
        image_extent: extent,
        image_array_layers: 1,
        //for now, we'll only use the swapchain as a framebuffer color attachment
        image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        //exclusive sharing between queue families has the best performance, but forces you to deal with ownership in between families if you use multiple ones.
        // we don't. we use one family without checking for presentation support. yay
        image_sharing_mode: vk::SharingMode::EXCLUSIVE,
        //          queue family infos are only needed if we're using CONCURRENT image sharing.
        //queue_family_index_count: ,
        //p_queue_family_indices: ,
        pre_transform: capabilities.current_transform,
        composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE, // todo!   INHERIT would be better but for some reason it's causing trouble on my PC (but not my laptop)
        present_mode: vk::PresentModeKHR::FIFO,
        //we don't care about obscured pixels (for now)
        clipped: vk::TRUE,
        //really quite pleasant that the ash bindings implement Default for pretty much all those structs
        ..Default::default()};

    let mut syn: Vec<SYN> = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
    //missing synchronization object creation error handling...
    (0..MAX_FRAMES_IN_FLIGHT).for_each(|_|unsafe { syn.push(SYN::new(device).unwrap()) });

    Ok((ext_swapchain.create_swapchain(&swapchain_create_info, None)?, format,extent, syn))
}


//this func was made due to code starting to be annoying to read in other files and a severe lack of proper error managment.
//ofc there's no real error management here too. crash and burn, shitheap
pub(crate) unsafe fn create_graphics_pipeline(device: &Device, extent: vk::Extent2D, render_pass: vk::RenderPass) -> (vk::Pipeline,vk::PipelineLayout) {
    let vertex_shader_code = load_shaders(include_str!("../shader/basic.vert"),shaderc::ShaderKind::Vertex);
    let fragment_shader_code = load_shaders(include_str!("../shader/basic.frag"),shaderc::ShaderKind::Fragment);
    let vsm_create_info = vk::ShaderModuleCreateInfo{
        //VERY IMPORTANT: the codesize is measured in BYTES.
        //however, the pointer to the code should be a *const u32 - a raw pointer to a 32bit unsigned integer
        code_size: vertex_shader_code.len(), //luckily, CompilationArtifact's .len() function returns the size in bytes
        p_code: vertex_shader_code.as_binary().as_ptr(),
        ..Default::default()};
    let fsm_create_info = vk::ShaderModuleCreateInfo{
        code_size: fragment_shader_code.len(),
        p_code: fragment_shader_code.as_binary().as_ptr(),
        ..Default::default()};
    let vertex_shader_module = device.create_shader_module(&vsm_create_info,None).unwrap();
    let fragment_shader_module = device.create_shader_module(&fsm_create_info,None).unwrap();


    let vss_create_info = vk::PipelineShaderStageCreateInfo {
        //flags:, for once, there's actually bitflags available. none of them i understand and none of them i need.
        stage: vk::ShaderStageFlags::VERTEX,
        module: vertex_shader_module,
        p_name: c"main".as_ptr(),
        //p_specialization_info: ,  for setting shader constants at runtime
        //if functionality changes depending on some const bool, setting the value for this at runtime instead of
        //  passing it as a push constant or uniform, allows some really good compiler optimizations
        ..Default::default()};
    let fss_create_info = vk::PipelineShaderStageCreateInfo {
        stage: vk::ShaderStageFlags::FRAGMENT,
        module: fragment_shader_module,
        p_name: c"main".as_ptr(),
        ..Default::default()};
    let stages = vec![vss_create_info,fss_create_info];

    //most of the pipeline is IMMUTABLE. depending on what you want to change you need to recreate the entire pipeline.
    //we can define some things as "dynamic-state", but then need to deal with thhose things when drawing.
    //for example the viewport size - because even though we already need to recreate the swapchain on window resize,
    //   there's no need to increase performance overhead by also recreating the pipeline.
    let dynamic_states = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state_info = vk::PipelineDynamicStateCreateInfo {
        dynamic_state_count: dynamic_states.len() as u32,
        p_dynamic_states: dynamic_states.as_ptr(),
        ..Default::default()};
    //vertices are hardcoded into the vertex shader, so we aint dealing with this stuff here. just some zeroes and nullpointers.
    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo {
        vertex_binding_description_count: 0,
        p_vertex_binding_descriptions: ptr::null(),
        vertex_attribute_description_count: 0,
        p_vertex_attribute_descriptions: ptr::null(),
        ..Default::default()};

    let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo {
        //index buffer info, effectively. nothing special needed for a singular tris.
        topology: vk::PrimitiveTopology::TRIANGLE_STRIP,
        primitive_restart_enable: vk::FALSE,
        ..Default::default()};

    //the viewport "scales" the area where we draw onto a swapchain image.
    //usually that's either the entire image, or a scaled-down version. you *can* make it stretched or something this way, though.
    let viewport = vk::Viewport {
        x: 0.0, y: 0.0,
        width: extent.width as f32,
        height: extent.height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
        ..Default::default()};
    //the "scissor" defines where on a swapchain image we can store data. everything outside is discarded.
    //pr much as if we'd draw normally on an image, then blit it onto a subsection of another image.
    //we define this as such we always draw onto the entire image. we do need to change those values on resize though.
    let scissor = vk::Rect2D {
        offset: vk::Offset2D {x:0,y:0}, extent };

    let pipeline_viewport_info = vk::PipelineViewportStateCreateInfo {
        viewport_count: 1,
        p_viewports: &viewport,
        scissor_count: 1,
        p_scissors: &scissor,
        ..Default::default()};

    //rasterization is BOTH IMPORTANT AND COOL, becasue you can do cool things with it
    let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
        depth_clamp_enable: vk::FALSE,
        rasterizer_discard_enable: vk::FALSE,
        polygon_mode: vk::PolygonMode::FILL,
        cull_mode: vk::CullModeFlags::NONE,
        depth_bias_enable: vk::FALSE, //don't properly understand + don't need to, YET
        line_width: 1.0,
        ..Default::default()};
    let multisample_info = vk::PipelineMultisampleStateCreateInfo {
        rasterization_samples: vk::SampleCountFlags::TYPE_1,
        sample_shading_enable: vk::FALSE,
        min_sample_shading: 1.0,
        p_sample_mask: ptr::null(),
        alpha_to_coverage_enable: vk::FALSE,
        alpha_to_one_enable: vk::FALSE,
        ..Default::default()};

    //don't care about blending as long as there's no overlapping geometry or backgrounds or whatever
    let blending_attachment_info = vk::PipelineColorBlendAttachmentState {
        color_write_mask: vk::ColorComponentFlags::RGBA,
        blend_enable: vk::FALSE,
/*
        src_color_blend_factor: vk::BlendFactor::ONE,
        dst_color_blend_factor: vk::BlendFactor::ONE,
        color_blend_op: vk::BlendOp::ADD,
        src_alpha_blend_factor: vk::BlendFactor::ONE,
        dst_alpha_blend_factor: vk::BlendFactor::ONE,
        alpha_blend_op: vk::BlendOp::MAX,*/
        ..Default::default()};
    let blending_info = vk::PipelineColorBlendStateCreateInfo {
        //there's a funny bitflag for custom blending as specified in a fragment shader. not yet implemented in my code.
        logic_op_enable: vk::FALSE,
        attachment_count: 1,
        p_attachments: &blending_attachment_info,
        blend_constants: [0.0;4],
        ..Default::default()};
    //we don't need uniforms, yet.
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo {
        set_layout_count: 0,
        p_set_layouts: ptr::null(),
        push_constant_range_count: 0,
        p_push_constant_ranges: ptr::null(),
        ..Default::default()};
    let pipeline_layout = device.create_pipeline_layout(&pipeline_layout_info,None).unwrap(); //error handling lmao


    let pipeline_info = vk::GraphicsPipelineCreateInfo {
        flags: Default::default(),
        stage_count: 2,
        p_stages: stages.as_ptr(),
        p_vertex_input_state: &vertex_input_info,
        p_input_assembly_state: &input_assembly_info,
        p_tessellation_state: ptr::null(),
        p_viewport_state: &pipeline_viewport_info,
        p_rasterization_state: &rasterization_info,
        p_multisample_state: &multisample_info,
        p_depth_stencil_state: ptr::null(),
        p_color_blend_state: &blending_info,
        p_dynamic_state: &dynamic_state_info,
        layout: pipeline_layout,
        render_pass,
        subpass: 0,
        base_pipeline_handle: vk::Pipeline::null(),
        base_pipeline_index: 0,
        ..Default::default()};

    let pipeline = device.create_graphics_pipelines(vk::PipelineCache::null(),&[pipeline_info],None).unwrap(); //you know the deal

    //only destroy shader modules once pipelines have been created
    device.destroy_shader_module(vertex_shader_module,None);
    device.destroy_shader_module(fragment_shader_module,None);

    (*pipeline.first().unwrap(),pipeline_layout)
}

//render passes tell vulkan what attachments we use as well as any important info regarding those
pub(crate) unsafe fn create_render_pass(device: &Device, format: vk::Format) -> vk::RenderPass {
    let color_attachment_desc = vk::AttachmentDescription {
        //there's a singular bitflag available here for aliasing attachments to one point in memory
        format,
        //likely needs to match multisampling sample count
        samples: vk::SampleCountFlags::TYPE_1,
        load_op: vk::AttachmentLoadOp::CLEAR,
        store_op: vk::AttachmentStoreOp::STORE,
        stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
        stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
        initial_layout: vk::ImageLayout::UNDEFINED, //only really important if we need previous frames information
        final_layout: vk::ImageLayout::PRESENT_SRC_KHR, //cuz we wanna put the stuff back into the swapchain for presentation
        ..Default::default()};
    //subpasses let us tell vulkan we want to do multiple rendering operations consecutively, where said operations use
    //  the output of previous subpasses as input. defining them as subpasses allows vulkan to make optimizations
    let subpass = vk::SubpassDescription {
        //flags: , ther's bitflags other than the usual placeholder. don't need them.
        pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        color_attachment_count: 1,
        p_color_attachments: &vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL },
        //p_input_attachments: , if we read from attachments in shaders
        //p_resolve_attachments: , stuff for multisampling
        //p_depth_stencil_attachment: ,
        //p_preserve_attachments: , unused attachments that we still want to preserve across subpasses
        ..Default::default()};

    let dependency = vk::SubpassDependency {
        //dependency_flags: ,
        src_subpass: vk::SUBPASS_EXTERNAL,
        dst_subpass: 0,
        src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        src_access_mask: vk::AccessFlags::NONE,
        dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ..Default::default()};

    let render_pass_info = vk::RenderPassCreateInfo {
        attachment_count: 1,
        p_attachments: &color_attachment_desc,
        subpass_count: 1,
        p_subpasses: &subpass,
        dependency_count: 1,
        p_dependencies: &dependency,
        ..Default::default()};
    let render_pass = device.create_render_pass(&render_pass_info,None).unwrap(); //error handlingn't
    render_pass
}

pub(crate) unsafe fn create_framebuffers(device: &Device, window: &Window, views: &Vec<vk::ImageView>, render_pass: vk::RenderPass) -> Vec<vk::Framebuffer> {
    let mut returnee: Vec<vk::Framebuffer> = Vec::with_capacity(views.len());
    for view in views {
        let framebuffer_info = vk::FramebufferCreateInfo {
            //flags: vk::FramebufferCreateFlags::IMAGELESS    commented out because this might end up useful to me some day
            render_pass,
            attachment_count: 1,
            p_attachments: ptr::from_ref(&view),
            width: window.inner_size().width,
            height: window.inner_size().height,
            layers: 1,
            ..Default::default()
        };
        returnee.push(device.create_framebuffer(&framebuffer_info,None).unwrap());  // todo!    ERROR HANDLING
    }
    returnee
}

pub(crate) unsafe fn record_into_buffer(device: &Device, window: &Window, pipeline: vk::Pipeline, render_pass: vk::RenderPass, framebuffer: vk::Framebuffer, extent: vk::Extent2D, command_buffer: vk::CommandBuffer, _image_index: u32) {
    let begin_info = vk::CommandBufferBeginInfo {
        //flags: vk::CommandBufferUsageFlags,
        p_inheritance_info: ptr::null(),
        ..Default::default()};
    device.begin_command_buffer(command_buffer, &begin_info).unwrap();

    //IMPORTANT: the color to which the screen is cleared
    let clear_value = vk::ClearValue {color: vk::ClearColorValue {float32:[0.0,0.0,0.0,0.0f32]}};
    let render_pass_info = vk::RenderPassBeginInfo {
        render_pass, framebuffer,
        render_area: vk::Rect2D::from(extent),
        clear_value_count: 1,
        p_clear_values: &clear_value,
        ..Default::default()};

    device.cmd_begin_render_pass(command_buffer,&render_pass_info,vk::SubpassContents::INLINE);

    device.cmd_bind_pipeline(command_buffer,vk::PipelineBindPoint::GRAPHICS,pipeline);

    //because we set the viewport and scissor as dynamic state previously, we gotta set them again.
    let viewport = vk::Viewport {
        x: 0.0, y: 0.0,
        width: window.inner_size().width as f32,
        height: window.inner_size().height as f32,
        min_depth: 0.0,
        max_depth: 1.0};
    device.cmd_set_viewport(command_buffer,0,&[viewport]);

    let scissor = vk::Rect2D::from(extent);
    device.cmd_set_scissor(command_buffer,0,&[scissor]);


    device.cmd_draw(command_buffer,4,1,0,0);
    device.cmd_end_render_pass(command_buffer);
    //because there aren't any errors thrown during command recording, everything that can go wrong will go wrong here.
    device.end_command_buffer(command_buffer).unwrap();
}


pub(crate) unsafe fn recreate_swapchain(
    device: &Device,
    physical_device: vk::PhysicalDevice,

    per_window: &mut PerWindow,

    ext_surface: &khr::surface::Instance,
    ext_swapchain: &khr::swapchain::Device,
) -> (vk::SwapchainKHR,Vec<vk::Image>,Vec<vk::ImageView>,Vec<vk::Framebuffer>,vk::Extent2D,Vec<SYN>) {

    device.device_wait_idle().unwrap(); // todo!

    // todo!   PASS OLD SWAPCHAIN TO NEW SWAPCHAIN CREATION AND WAIT WITH SWAPCHAIN CLEANUP UNTIL THERE'S NO MORE FRAMES IN FLIGHT OF THE OLD ONE
    swapchain_cleanup(device,ext_swapchain,per_window.swapchain,&per_window.views,&per_window.framebuffers,&per_window.synchronization);

    let (swapchain,format,extent,syn) = create_swapchain(&per_window.window, per_window.surface, device, physical_device, ext_surface, ext_swapchain).unwrap(); // todo!
    let images = ext_swapchain.get_swapchain_images(swapchain).unwrap();    // todo!
    let views = create_views(device,&images,format);
    let framebuffers: Vec<vk::Framebuffer> = create_framebuffers(device,&per_window.window,&views,per_window.render_pass);

    (swapchain,images,views,framebuffers,extent,syn)
}

pub(crate) unsafe fn swapchain_cleanup(
    device: &Device,
    ext_swapchain: &khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    views: &Vec<vk::ImageView>,
    framebuffers: &Vec<vk::Framebuffer>,
    synchronization: &Vec<SYN>,
) {
    for syn in synchronization {
        syn.destroy(device);
    }
    for framebuffer in framebuffers {
        device.destroy_framebuffer(*framebuffer, None);
    }
    for view in views {
        device.destroy_image_view(*view,None);
    }
    ext_swapchain.destroy_swapchain(swapchain,None);
}

pub(crate) unsafe fn create_views(device: &Device, images: &Vec<vk::Image>, format: vk::Format) -> Vec<vk::ImageView> {
    let mut views: Vec<vk::ImageView> = Vec::with_capacity(images.len());
    for image in images {
        let view_create_info = vk::ImageViewCreateInfo {
            //flags: ,
            image: *image,format,
            view_type: vk::ImageViewType::TYPE_2D,
            components: vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY},
            subresource_range: vk::ImageSubresourceRange {
                //lot of interesting stuff regarding image aspect flags/masks - but nothing important for now.
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },..Default::default()};
        views.push(device.create_image_view(&view_create_info, None).unwrap()); // todo!    ERROR HANDLING
        }
    views
}