use std::ffi::{c_char, CStr};
use ash::vk;
/*fn from_available<const N1: usize, const N2: usize, T>(
    available: Vec<T>,
    parser: fn(T) -> &CStr,
    required: [&CStr;N1],
    optional: [&CStr;N2]
) -> Vec<*const c_char> {
    let
    let available: Vec<&CStr> = available.iter().map(parser).collect();
    for requirement in required {
        if available.contains(requirement) {

        }
    }
    for option in optional {

    }
}*/

struct Available<const N1: usize, const N2: usize, T> {
    available: Vec<T>,
    required: [&'static CStr;N1],
    optional: [&'static CStr;N2]
}
trait ToCStr<'a,T> {
    fn to_cstr(self: Vec<&'a T>) -> Vec<&'a CStr> {

    }
}
impl ToCStr for Vec<>
impl<const N1: usize, const N2: usize, T> Iterator for Available<N1,N2,T> {
    type Item = ();
    fn next(&mut self) -> Option<Self::Item> {
    }
}