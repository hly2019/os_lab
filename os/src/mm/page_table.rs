//! Implementation of [`PageTableEntry`] and [`PageTable`].

use super::{frame_alloc, FrameTracker, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;
use crate::task::current_user_token;
pub use crate::mm::*;
bitflags! {
    /// page table entry flags
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
/// page table entry structure
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }

    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
    pub fn PTE_U(&self) -> bool {
        (self.flags() & PTEFlags::U) != PTEFlags::empty()
    }
}


/// page table structure
pub struct PageTable {
    pub root_ppn: PhysPageNum,
    pub frames: Vec<FrameTracker>,
}

/// Assume that it won't oom when creating/mapping.
impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
    /// Temporarily used to get arguments from user space.
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }
    pub fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let mut idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter_mut().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }
    #[allow(unused)]
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) -> bool {
        let pte = self.find_pte_create(vpn).unwrap();
        // assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        if pte.is_valid() {
            return false;
        }
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
        return true;
        // let pte = self.find_pte_create(vpn).unwrap();
        // assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        // *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
        // unsafe {
        //     if vpn.0 != 65536 {
        //         return;
        //     }
        //     println!("test addr {}", vpn.0 );
            
        //     *((ppn.0 * 4096) as *mut u8) = 1;
        //     println!("xaaaaaaaaaaaaaaaaaaaaaaaaaai");
        // }

    }
    #[allow(unused)]
    pub fn unmap(&mut self, vpn: VirtPageNum) -> bool {
        let pte = self.find_pte_create(vpn).unwrap();
        // assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        if pte.is_valid() {
            return false;
        }
        *pte = PageTableEntry::empty();
        return true;

    }
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).copied()
    }
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}




/// translate a pointer to a mutable u8 Vec through page table
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}
pub fn my_map(vpn: VirtPageNum, ppn: PhysPageNum, permission: MapPermission, token: usize) -> bool{
    let mut page_table = PageTable::from_token(token);
    true
    // page_table.my_map(vpn, ppn, flags)
    // if let Some(pt) = page_table.find_pte(vpn) {
    //     // println!("has pt: {}", pt.ppn().0);
    //     if pt.is_valid(){
    //         return false;
    //     }
    // }
    // let pte = page_table.find_pte_create(vpn).unwrap();
    // println!("Before: vpn is : {}, pte bits is: {}, pte valid is {}", vpn.0, pte.bits, pte.is_valid());
    // println!("is valid?: {}, vpn: {}, curuser: {}", pte.ppn().0, vpn.0, current_user_token());
    // if pte.is_valid() { // pte is valid, error
    //     println!("is already valid");
    //     return false;
    // }
    // page_table.map(vpn, ppn, flags);
    // assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
    // *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    // println!("After: vpn is : {}, pte bits is: {}, pte valid is {}", vpn.0, pte.bits, pte.is_valid());
    // println!("after is valid?: {}, vpn: {}, curuser: {}", pte.ppn().0, vpn.0, current_user_token());

    // true
}

pub fn my_munmap(vpn: VirtPageNum, token: usize) -> bool {
    let page_table = PageTable::from_token(token);
    // let pte = page_table.find_pte(vpn).unwrap();
    // // assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
    // if !pte.is_valid() {
    //     return false;
    // }
    // pte = PageTableEntry::empty();
    if let Some(mut pte) = page_table.find_pte(vpn) {
        if !pte.is_valid() {
            return false;
        }
        println!("pte: {}, vpn: {}", pte.ppn().0, vpn.0);
        pte = &mut PageTableEntry::empty();
    }
    else {
        return false;
    }
    true
}

