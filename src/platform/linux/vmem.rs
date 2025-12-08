use std::ffi::c_void;
use std::fs::File;
use std::io::Read;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::{mem, ptr, slice};
use bitflags::bitflags;


pub(crate) struct VmMapping {
    inner: Vec<VmMapEntry>,
}
pub(crate) struct VmMapEntry {
    pub address: (*mut c_void, *mut c_void),    // (64bit,64bit) = 128bit
    pub permissions: Permissions,   //u8
    pub offset: u64,
    pub device: (u16,u16),    // hex as  ma:mi major-minor
    pub inode: u32,
    pub pathname: VmPath,
}
pub enum VmPath {
    PATH(String), //a path.
    UNKNOWN, // nothing. pr much exclusively due to mmap()
    DELETED(String), //a path with (deleted) appended.

    STACK, //[stack]
    VDSO, //[vdso]
    HEAP, //[heap]
    ANON(String), //anon:name               NEVER SEEN?
    ANON_SHMEM(String), //anon_shmem:name   NEVER SEEN?

    ANON_INODE(String), //anon_inode:name

    VVAR, //[VVAR]
    VVAR_VCLOCK, //[VVAR_VCLOCK]
    VSYSCALL, //[VSYSCALL]


}
pub struct Permissions(u8);
bitflags! {
    impl Permissions: u8 {
        const PRIVATE = 0;
        const  SHARED = 1 << 0;
        const    READ = 1 << 1;
        const   WRITE = 1 << 2;
        const EXECUTE = 1 << 3;
    }
}
impl Permissions {
    pub fn serialize(&self) -> String {
        let mut string = String::with_capacity(4);
        if self.contains(Self::READ) {
            string.push('r');
        } else { string.push('-') }
        if self.contains(Self::WRITE) {
            string.push('w');
        } else { string.push('-') }
        if self.contains(Self::EXECUTE) {
            string.push('x');
        } else { string.push('-') }
        if self.contains(Self::SHARED) {
            string.push('s');
        } else { string.push('p') }
        string
    }
    pub fn deserialize(string: &str) -> Self {
        let mut chars = string.chars();
        let mut permissions = Self::empty();
        if chars.next().unwrap() == 'r' {
            permissions |= Self::READ;
        }
        if chars.next().unwrap() == 'w' {
            permissions |= Self::WRITE;
        }
        if chars.next().unwrap() == 'x' {
            permissions |= Self::EXECUTE;
        }
        if chars.next().unwrap() == 's' {
            permissions |= Self::SHARED;
        }
        permissions
    }
}

impl Display for Permissions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.serialize().as_str())
    }
}
impl Debug for Permissions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_string().as_str())
    }
}
impl Display for VmMapping {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("VmMapping {\n")?;
        for entry in self.inner.iter() {
            f.write_fmt(format_args!("    {}\n", entry))?
        }
        f.write_str("}")
    }
}
impl Debug for VmMapping {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_string().as_str())
    }
}
impl Display for VmMapEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:>16x}-{:<16x} ", self.address.0.addr(), self.address.1.addr()))?;
        f.write_str(self.permissions.serialize().as_str())?;
        f.write_fmt(format_args!(" {:0<10} ", self.offset))?;
        f.write_fmt(format_args!("{:0<2x}:{:0<2x} ", self.device.0, self.device.1))?;
        f.write_fmt(format_args!("{:<8} ", self.inode))?;
        f.write_fmt(format_args!("{}", self.pathname))
    }
}
impl Debug for VmMapEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_string().as_str())
    }
}
impl Display for VmPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UNKNOWN => f.write_str(""),
            Self::PATH(string) => f.write_str(string.as_str()),
            Self::ANON(string) => f.write_fmt(format_args!("anon:{}", string)),
            Self::ANON_SHMEM(string) => f.write_fmt(format_args!("anon_shmem:{}", string)),
            Self::HEAP => f.write_str("[heap]"),
            Self::STACK => f.write_str("[stack]"),
            Self::VDSO => f.write_str("[vdso]"),
            Self::DELETED(string) => f.write_fmt(format_args!("{} (deleted)", string)),
            Self::ANON_INODE(string) => f.write_fmt(format_args!("anon_inode:{}", string)),
            Self::VVAR => f.write_str("[vvar]"),
            Self::VVAR_VCLOCK => f.write_str("[vvar_vclock]"),
            Self::VSYSCALL => f.write_str("[vsyscall]"),
        }
    }
}
impl Debug for VmPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_string().as_str())
    }
}

impl VmMapping {
    pub unsafe fn from_pid(pid: usize) -> Self {
        let mut buf = String::new();
        // require root permissions
        File::open(&format!("/proc/{}/maps", pid))
            .unwrap_or_else(|e| panic!("cannot open file (require permissions?)\n{:?}", e))
            .read_to_string(&mut buf).unwrap();

        Self {
            inner: buf
                .split_terminator('\n')
                .map(|data| {
                let mut data = data.split(' ');
                VmMapEntry {
                    address: {
                        let addr = data.next().unwrap().split_once('-').unwrap();
                        (usize::from_str_radix(addr.0, 16).unwrap() as *mut c_void,
                         usize::from_str_radix(addr.1, 16).unwrap() as *mut c_void)
                    },
                    permissions: Permissions::deserialize(data.next().unwrap()),
                    offset: u64::from_str_radix(data.next().unwrap(), 16).unwrap(),
                    device: {
                        let device = data.next().unwrap().split_once(':').unwrap();
                        (u16::from_str_radix(device.0, 16).unwrap(),
                         u16::from_str_radix(device.1, 16).unwrap())
                    },
                    inode: data.next().unwrap().parse::<u32>().unwrap(),
                    pathname: {
                        let path_raw = data.last().unwrap();
                        if let Some(idx) = path_raw.find(':') {
                            let path = path_raw[idx+1..path_raw.len()].to_string();
                            if idx == 4 {
                                VmPath::ANON(path)
                            } else {
                                match &path_raw[5..10] {
                                    "shmem" => VmPath::ANON_SHMEM(path),
                                    "inode" => VmPath::ANON_INODE(path),
                                    &_ => unreachable!()
                                }
                            }
                        } else {
                            match path_raw {
                                "[vvar]" => VmPath::VVAR,
                                "[vvar_vclock]" => VmPath::VVAR_VCLOCK,
                                "[vsyscall]" => VmPath::VSYSCALL,
                                "[stack]" => VmPath::STACK,
                                "[vdso]" => VmPath::VDSO,
                                "[heap]" => VmPath::HEAP,
                                "" => VmPath::UNKNOWN,
                                path => {
                                    if let Some(idx) = path.rfind("(deleted)") {
                                        VmPath::DELETED(path[..idx.saturating_sub(1)].to_string())
                                    } else {
                                        VmPath::PATH(path.to_string())
                                    }
                                }
                            }}
                    }}})
                .collect::<Vec<VmMapEntry>>()
        }
    }
}

impl Deref for VmMapping {
    type Target = Vec<VmMapEntry>;
    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

pub(crate) struct PTrace {
    pid: i32,
    base: *const c_void,
}

impl PTrace {
    pub fn new(pid: i32, base: *const c_void) -> Self {
        Self { pid, base }
    }
    #[inline]
    pub unsafe fn seize(&mut self) {
        //todo! void* data bitmask in 4th arg of ptrace call
        libc::ptrace(
            libc::PTRACE_SEIZE,
            self.pid,
            ptr::null::<c_void>(),
            ptr::null::<c_void>()
        );
    }
    #[inline]
    pub unsafe fn peek(&self, offset: isize) -> u16 {
        libc::ptrace(
            libc::PTRACE_PEEKDATA,
            self.pid,
            self.base.byte_offset(offset),
            ptr::null::<c_void>()
        ) as u16
    }
    #[inline]
    pub unsafe fn poke<T>(&self, offset: isize, data: &T) {
        libc::ptrace(
            libc::PTRACE_POKEDATA,
            self.pid,
            self.base.byte_offset(offset),
            ptr::from_ref(data)
        );
    }
    #[inline]
    pub unsafe fn peek_user(&self, offset: isize) -> u16 {
       libc::ptrace(
           libc::PTRACE_PEEKUSER,
           self.pid,
           offset,
           ptr::null::<c_void>()
       ) as u16
   }
    //todo! maybe proc macro to annotate this is giga unsafe
    #[inline]
    pub unsafe fn poke_user<T>(&self, offset: isize, data: &T) {
        libc::ptrace(
            libc::PTRACE_POKEUSER,
            self.pid,
            offset,
            ptr::from_ref(data)
        );
    }

    pub unsafe fn yoink<T>(&self, mut offset: isize) -> T {
        let mut raw = mem::zeroed::<T>();
        let mut slice = slice::from_raw_parts_mut(
            ptr::from_mut(&mut raw).cast::<u16>(),
            size_of::<T>() / size_of::<u16>());
        slice.fill_with(|| {
            let tmp = self.peek(offset);
            offset += size_of::<u16>() as isize;
            return tmp;
        });
        raw
    }

}