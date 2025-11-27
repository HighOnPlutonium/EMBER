#![warn(unsafe_op_in_unsafe_fn)]

use libc::{iovec, process_vm_readv, ptrace, PTRACE_PEEKDATA, PTRACE_SEIZE};
use std::{ffi::c_void, fs::File, io::Read, mem, process::Command, ptr, slice};
use errno::errno;
use log::{error, trace};

//  todo!   implement vmem.rs to make this here redundant.
/// relies on reading "/proc/{pid}/maps", root access might be needed. 
pub unsafe fn libkwin_base_address(pid: usize) -> *mut c_void {
    let mut buffer = String::new();
    // require root permissions
    File::open(&format!("/proc/{}/maps", pid.0))
        .unwrap_or_else(|e| panic!("cannot open file (require permissions?)\n{:?}", e))
        .read_to_string(&mut buffer)
        .unwrap();
    
    let start_idx = buffer[..buffer.find(r#"libkwin.so"#).unwrap()].rfind('\n').unwrap();
    let mut end_idx = buffer.rfind(r#"libkwin.so"#).unwrap();
    end_idx = buffer[end_idx..].find('\n').unwrap() + end_idx;
    let subslice: Vec<String> = buffer[start_idx..end_idx].split_once('\n').unwrap().1.split('\n').map(str::to_owned).collect();
    
    subslice.iter().for_each(|x|trace!("{}",x));

    return usize::from_str_radix(&subslice[0][..12], 16).unwrap() as *mut c_void;
}

pub fn get_mouse_pos(pid: usize, base: *mut c_void, offset: usize) -> [u8; 16] {
    let mut cursors_addr = ptr::null_mut::<c_void>();
    let local = iovec {
        iov_base: ptr::from_mut(&mut cursors_addr).cast(),
        iov_len: 8,
    };
    let remote = iovec {
        iov_base: unsafe { base.byte_add(offset) },
        iov_len: 8,
    };
    //public field Cursors::s_self              !!! ptr !!!
    match unsafe { libc::process_vm_readv(pid, &local, 1, &remote, 1, 0) } {
        8 => assert!(!cursors_addr.is_null()),
        -1 => {
            error!("{}", errno());
        }
        _ => unreachable!()
    }

    let mut cursor_addr: *mut c_void = ptr::null_mut();
    let local = iovec {
        iov_base: ptr::from_mut(&mut cursor_addr).cast(),
        iov_len: 8,
    };
    let remote = iovec {
        iov_base: unsafe { cursors_addr.byte_add(16) },
        iov_len: 8,
    };
    //private field Cursors.m_currentCursor     !!! ptr !!!
    match unsafe { libc::process_vm_readv(pid, &local, 1, &remote, 1, 0) } {
        8 => assert!(!cursor_addr.is_null()),
        -1 => {
            error!("{}", errno());
        }
        x => unreachable!()
    }

    let mut pos = [0u8; 16];
    let local = iovec {
        iov_base: ptr::from_mut(&mut pos).cast(),
        iov_len: 16,
    };
    let remote = iovec {
        iov_base: unsafe { cursor_addr.byte_add(32) },
        iov_len: 16,
    };
    //private field Cursor.m_pos
    match unsafe { libc::process_vm_readv(pid, &local, 1, &remote, 1, 0) } {
        16 => (),
        -1 => {
            error!("{}", errno());
        }
        x => unreachable!()
    }
    pos
}