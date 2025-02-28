use std::marker::PhantomData;
use std::ptr;
use ash::Entry;
use ash::vk;

fn main() {
    let entry = unsafe { Entry::load().unwrap() };
    let app_info = vk::ApplicationInfo {
        p_application_name: "placeholder name".as_ptr().cast(),
        //application_version: 0,
        //p_engine_name: (),
        //engine_version: 0,
        api_version: vk::make_api_version(0,1,0,0),
        ..Default::default()
    };
    
    let create_info = vk::InstanceCreateInfo {

        p_application_info: &app_info,
        //enabled_layer_count: 0,
        //pp_enabled_layer_names: (),
        //enabled_extension_count: 0,
        //pp_enabled_extension_names: (),
        ..Default::default()
    };
    
    let instance = unsafe { entry.create_instance(&create_info, None).unwrap() };
}
