use easy_fs::{
    EasyFileSystem,
    Inode,
};
use crate::{drivers::BLOCK_DEVICE, task::current_user_token};
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use lazy_static::*;
use bitflags::*;
use alloc::vec::Vec;
use super::{File, Stat, StatMode};
use crate::mm::{UserBuffer, PageTable, VirtAddr};

/// A wrapper around a filesystem inode
/// to implement File trait atop
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}

/// The OS inode inner in 'UPSafeCell'
pub struct OSInodeInner {
    offset: usize,
    pub inode: Arc<Inode>,
}

impl OSInode {
    /// Construct an OS inode from a inode
    pub fn new(
        readable: bool,
        writable: bool,
        inode: Arc<Inode>,
    ) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner {
                offset: 0,
                inode,
            })},
        }
    }
    /// Read all data inside a inode into vector
    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }
}

lazy_static! {
    /// The root of all inodes, or '/' in short
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

/// List all files in the filesystems
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/");
}

bitflags! {
    /// Flags for opening files
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
    }
}

impl OpenFlags {
    /// Get the current read write permission on an inode
    /// does not check validity for simplicity
    /// returns (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}


pub fn linkat(oldname: &str, newname: &str) -> isize {
    return ROOT_INODE.linkat(oldname, newname);
}

pub fn unlinkat(name: &str) -> isize {
    return ROOT_INODE.unlinkat(name);
}

/// Open a file by path
pub fn open_file(name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = ROOT_INODE.find(name) {
            // clear size
            inode.clear();
            Some(Arc::new(OSInode::new(
                readable,
                writable,
                inode,
            )))
        } else {
            // create file
            ROOT_INODE.create(name)
                .map(|inode| {
                    Arc::new(OSInode::new(
                        readable,
                        writable,
                        inode,
                    ))
                })
        }
    } else {
        ROOT_INODE.find(name)
            .map(|inode| {
                if flags.contains(OpenFlags::TRUNC) {
                    inode.clear();
                }
                Arc::new(OSInode::new(
                    readable,
                    writable,
                    inode
                ))
            })
    }
}

impl File for OSInode {
    fn readable(&self) -> bool { self.readable }
    fn writable(&self) -> bool { self.writable }
    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.offset, *slice);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }
    fn write(&self, buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }

    fn get_state(&self, buf: *mut Stat) {
        let mut inner = self.inner.exclusive_access();
        let token = current_user_token();
        let page_table = PageTable::from_token(token);
        let start_va = VirtAddr::from(buf as usize);
        let vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn().0;
        let ptr = ppn << 12 | start_va.page_offset() as usize; // ppn左移12位拼上offset
        unsafe {
            (*(ptr as *mut Stat)).dev = 0;
            (*(ptr as *mut Stat)).pad = [0u64; 7];
            // ROOT_INODE;
            (*(ptr as *mut Stat)).ino = inner.inode.as_ref().get_inode_id() as u64;
            println!("inode in is {}", (*(ptr as *mut Stat)).ino);
            // (*(ptr as *mut Stat)).mode =
            if  inner.inode.as_ref().get_disk_inode_is_dic() {
                (*(ptr as *mut Stat)).mode =StatMode::DIR;
            }
            else {
                (*(ptr as *mut Stat)).mode =StatMode::FILE;
            }
            println!("id: {}", (*(ptr as *mut Stat)).ino);
            println!("real inode number is: {}", ROOT_INODE.get_id_by_name("fname2"));
            (*(ptr as *mut Stat)).nlink = ROOT_INODE.countlink((*(ptr as *mut Stat)).ino as u32);
            println!("inode nlink is {}", (*(ptr as *mut Stat)).nlink);

        }
    }
}
