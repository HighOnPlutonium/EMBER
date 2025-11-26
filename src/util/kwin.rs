#![warn(unsafe_op_in_unsafe_fn)]

pub mod pointer {

    use libc::{iovec, process_vm_readv, ptrace, PTRACE_PEEKDATA, PTRACE_SEIZE};
    use std::{ffi::c_void, fs::File, io::Read, mem, process::Command, ptr, slice};
    use errno::errno;

    #[derive(Clone, Copy, Eq, PartialEq, Debug)]
    pub struct KWinPid(i32);
    impl KWinPid {
        pub unsafe fn from(i: i32) -> Self {
            unsafe {
                if libc::getuid() != 0 {
                    // if is not root
                    if libc::setuid(0) != 0 {
                        // if cannot be root
                        panic!("cannot set uid to 0, further code could not be executed.")
                    }
                }
            }
            Self(i)
        }
        pub unsafe fn search(all_user: bool) -> Self {
            Self::from(
                String::from_utf8_lossy(
                    &Command::new("ps")
                        .arg(if all_user { "ax" } else { "x" }) // "a" is needed since there might not be a wayland window running by root.
                        .output()
                        .expect("cannot enumerate programs")
                        .stdout,
                )
                    .lines()
                    .filter(|x| x.contains("/kwin_wayland "))
                    .next()
                    .expect("failed to find kwin_wayland session")
                    .trim()
                    .split_once(' ')
                    .expect("cannot parse `ps`'s output")
                    .0
                    .parse()
                    .expect("cannot parse the pid")
            )
        }
    }


    mod virtual_memory {
        use std::ffi::c_void;
        use std::path::PathBuf;

        enum Permissions {
            Read,
            Write,
            Execute,
            Shared
        }
        enum MemoryType {
            Anonymous,  // not associated with any file
            File,       //
            Heap,       //
            Stack,      //
            VDyn,       // VDSO - virtual dynamic shared object
            VVar,       // idk
            VClock,     // idk
            VSysCall,   //  idk
        }
        struct Source {
            device: String,
            inode: usize,
            pathname: Option<PathBuf>,
            memory_type: MemoryType,
        }
        struct ContiguousVMRegion {
            address: *mut c_void,
            length: usize,
            perms: Permissions,
            offset: usize,
            source: Source,
        }


    }



    pub(crate) struct Aux {
        pub(crate) cursors: usize,     //  Cursors::s_self
        pub(crate) workspace: usize,   //  Workspace::_self
    }

    pub struct KWin(KWinPid, pub *mut c_void, pub Aux);
    impl KWin {
        /// relies on reading "/proc/{pid}/maps", root access might be needed.
        pub fn get(pid: KWinPid) -> Self {
            let mut buffer = String::new();
            // require root permissions
            File::open(&format!("/proc/{}/maps", pid.0))
                .unwrap_or_else(|e| panic!("cannot open file (require permissions?)\n{:?}", e))
                .read_to_string(&mut buffer)
                .unwrap();

            let libkwin_start_idx = buffer[..buffer.find(r#"libkwin.so"#).unwrap()].rfind('\n').unwrap();
            let mut libkwin_end_idx = buffer.rfind(r#"libkwin.so"#).unwrap();
            libkwin_end_idx = buffer[libkwin_end_idx..].find('\n').unwrap() + libkwin_end_idx;
            let libkwin_subslice: Vec<String> = buffer[libkwin_start_idx..libkwin_end_idx].split_once('\n').unwrap().1.split('\n').map(str::to_owned).collect();
            libkwin_subslice.iter().for_each(|x|println!("{}",x));
            let addr = usize::from_str_radix(&libkwin_subslice[0][..12], 16).unwrap() as *mut c_void;
            Self(pid, addr, Aux {
                cursors: Self::get_offset_with_readelf("readelf", "/usr/lib64/libkwin.so.6.4.5", "Cursors::s_self"),
                workspace: Self::get_offset_with_readelf("readelf", "/usr/lib64/libkwin.so.6.4.5", "Workspace::_self"),
            })
        }

        pub fn get_offset_with_readelf(readelf: &str, path_to_libkwin: &str, symbol: &str) -> usize {
            usize::from_str_radix(
                &String::from_utf8(
                    Command::new(readelf)
                        .args(["-WCs", path_to_libkwin])
                        .output()
                        .expect("readelf execute failed")
                        .stdout,
                )
                    .expect("failed to parse readelf")
                    .split_once(symbol)
                    .expect(&format!("cannot find symbol \"{}\"",symbol))
                    .0
                    .rsplit_once('\n')
                    .expect(&format!("cannot read offset of \"{}\"",symbol))
                    .1
                    .split_once(':')
                    .expect("parse `:` failed.")
                    .1
                    .trim()
                    .split_once(' ')
                    .expect("cannot parse space")
                    .0,
                16,
            )
                .expect("failed to process readelf.")
        }

        pub fn get_mouse_pos(&self) -> [u8; 16] {
            let mut cursors_addr: *mut c_void = ptr::null_mut();
            let local = iovec {
                iov_base: ptr::from_mut(&mut cursors_addr).cast(),
                iov_len: 8,
            };
            let remote = iovec {
                iov_base: unsafe { self.1.byte_add(self.2.cursors) },
                iov_len: 8,
            };
            //public field Cursors::s_self              !!! ptr !!!
            match unsafe { process_vm_readv(self.0.0, &local, 1, &remote, 1, 0) } {
                8 => assert!(!cursors_addr.is_null()),
                -1 => {
                    eprintln!("failed, check errno for more details.");
                    dbg!(&errno());
                }
                x => eprintln!("unknown bytes read: {x}"),
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
            match unsafe { process_vm_readv(self.0.0, &local, 1, &remote, 1, 0) } {
                8 => assert!(!cursor_addr.is_null()),
                -1 => {
                    eprintln!("failed, check errno for more details.");
                    dbg!(&errno());
                }
                x => eprintln!("unknown bytes read: {x}"),
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
            match unsafe { process_vm_readv(self.0.0, &local, 1, &remote, 1, 0) } {
                16 => (),
                -1 => {
                    eprintln!("failed, check errno for more details.");
                    dbg!(&errno());
                }
                x => eprintln!("unknown bytes read: {x}"),
            }
            pos
        }

        pub fn get_stuff(&self) -> [u8; 512] {

            let offset = Self::get_offset_with_readelf("readelf", "/usr/lib64/libkwin.so.6.4.5", "Workspace::_self");



            let mut addr: *mut c_void = ptr::null_mut();
            let local = iovec {
                iov_base: ptr::from_mut(&mut addr).cast(),
                iov_len: 8,
            };
            let remote = iovec {
                iov_base: unsafe { self.1.byte_add(offset).byte_add(8) },
                iov_len: 8,
            };
            match unsafe {
                process_vm_readv(self.0.0, &local, 1, &remote, 1, 0)
            } {
                8 => assert!(!addr.is_null()),
                -1 => eprintln!("{:?}",&errno()),
                _ => unreachable!()
            }
            println!("\nptr (addr) after offset+8: {:?}",addr);


            let mut addr2: *mut c_void = ptr::null_mut();
            let local = iovec {
                iov_base: ptr::from_mut(&mut addr2).cast(),
                iov_len: 8,
            };
            let remote = iovec {
                iov_base: unsafe { addr.byte_add(0) },
                iov_len: 8,
            };
            match unsafe {
                process_vm_readv(self.0.0, &local, 1, &remote, 1, 0)
            } {
                8 => assert!(!addr2.is_null()),
                -1 => eprintln!("{:?}",&errno()),
                _ => unreachable!()
            }
            println!("ptr (addr2) at *addr: {:?}",addr2);



            let mut data = [0u8; 512];

            let local = iovec {
                iov_base: ptr::from_mut(&mut data).cast(),
                iov_len: 512,
            };
            let remote = iovec {
                iov_base: unsafe { addr2.byte_add(0) },
                iov_len: 512,
            };

            match unsafe { process_vm_readv(self.0.0, &local, 1, &remote, 1, 0) } {
                512 => (),
                -1 => {
                    eprintln!("failed, check errno for more details.");
                    dbg!(&errno());
                }
                x => eprintln!("unknown bytes read: {x}"),
            }
            let idx = 2;
            let val = unsafe {mem::transmute_copy::<_,*const c_void,>(&data.as_chunks_unchecked::<8>()[idx])};
            let next = unsafe {mem::transmute_copy::<_,*const c_void,>(&data.as_chunks_unchecked::<8>()[idx+1])};
            println!("qword at *addr2+8*{}: {:?}",idx,val);
            println!("delta: {:?}",
                     unsafe { mem::transmute::<_,*const c_void>(mem::transmute::<_,u64>(next) as i64 - mem::transmute::<_,u64>(val) as i64) }
            );
            data
        }

        pub unsafe fn lens(&self) -> [u64; 4096] {

            let offset = Self::get_offset_with_readelf("readelf", "/usr/lib64/libkwin.so.6.4.5", "Workspace::_self");

            const N: usize = 4096;
            const N_BYTES: isize = 8*N as isize;
            let mut data: [u64; N] = mem::zeroed::<[u64; N]>();
            let local = iovec {
                iov_base: ptr::from_mut(&mut data).cast(),
                iov_len: N*8,
            };
            let remote = iovec {
                iov_base: unsafe { self.1.byte_add(offset).byte_add(0).byte_sub(64) },
                iov_len: N*8,
            };
            match unsafe {
                process_vm_readv(self.0.0, &local, 1, &remote, 1, 0)
            } {
                N_BYTES => (),
                -1 => eprintln!("{:?}",&errno()),
                _ => unreachable!()
            }

            data

        }
    }
}