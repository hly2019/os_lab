//! Implementation of [`MapArea`] and [`MemorySet`].

use super::{frame_alloc, FrameTracker};
use super::{PTEFlags, PageTable, PageTableEntry};
use super::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use super::{StepByOne, VPNRange};
use crate::config::{MEMORY_END, PAGE_SIZE, TRAMPOLINE, TRAP_CONTEXT, USER_STACK_SIZE};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;
use riscv::register::satp;
use spin::Mutex;

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

lazy_static! {
    /// a memory set instance through lazy_static! managing kernel space
    pub static ref KERNEL_SPACE: Arc<Mutex<MemorySet>> =
        Arc::new(Mutex::new(MemorySet::new_kernel()));
}

/// memory set structure, controls virtual-memory space
pub struct MemorySet {
    page_table: PageTable,
    areas: Vec<MapArea>,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }
    pub fn token(&self) -> usize {
        self.page_table.token()
    }
    /// Assume that no conflicts.
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        println!("start va before insert maparea: {}", start_va.0 / 4096);
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
        unsafe {
            // *(start_va.0 as *mut u8) = 1;
            // println!("xaaaaaaaaaaaaaaaaaaaaaaaaaai");
        }

    }

    pub fn cancel_framed_area(&mut self, start: VirtAddr, end: VirtAddr) ->bool {
        let mut flag = 0;
        for i in 0..self.areas.len(){
            let area = &self.areas[i];
            // let area_start = area.vpn_range.get_start();
            // let area_end = area.vpn_range.get_end();
            if !(area.vpn_range.get_start() >= end.ceil() || area.vpn_range.get_end() <= start.floor()) {
                // 相交
                flag = i;
                break;
            }
        }
        if flag == self.areas.len() {
            panic!("error, illegal page");
        }
        // self.unmap()
        let mut ret = true;
        for vpn in start.floor().0..end.ceil().0 {
            ret &= self.areas[flag].unmap_one(&mut self.page_table, VirtPageNum::from(vpn));
            if ret == false {
                println!("vpn is: {}", vpn)
            }

        }
        self.areas.remove(flag);
        println!("in memory set");
        return ret;
        // self.areas[flag].unmap(&mut self.page_table);
        // self.areas[flag].vpn_range = VPNRange::new(VirtPageNum::from(0),VirtPageNum::from(0));
    }



    pub fn judge_map_right(&mut self, start_va: VirtPageNum, end_va: VirtPageNum) -> bool {
        let mut start = start_va.0;
        while start < end_va.0 {
            let mut flag = true;
            for i in 0..self.areas.len() {
                let mut area: &MapArea = &self.areas[i];
                if VirtPageNum::from(area.vpn_range.get_start().0).0 <= start 
                && start < VirtPageNum::from(area.vpn_range.get_end().0).0 {
                    println!("jajajajajajajaja {}, start is {}", VirtPageNum::from(area.vpn_range.get_start().0).0,start);
                    // println!("i is {},start is: {}, contain?: {}",i, start, self.areas[i].data_frames.contains_key(&VirtPageNum::from(start)));
                    // for key in self.areas[i].data_frames.keys() {
                    //     println!("key is {}", key.0);
                    // }
                    if self.areas[i].data_frames.contains_key(&VirtPageNum::from(start)) { // some maparea contains start, ok
                        flag = false;
                    }
                }
            }
            if !flag { // no maparea contains start, fail.
                println!("return: {}", false);
                return false;
            }
            start += 1;
        }
        return true
    }

    pub fn judge_unmap_right(&mut self, start_va: VirtPageNum, end_va: VirtPageNum) -> bool {
        let mut start = start_va.0;
        while start < end_va.0 {
            let mut flag = false;
            for i in 0..self.areas.len() {
                let area: &MapArea = &self.areas[i];
                if self.areas[i].data_frames.contains_key(&VirtPageNum::from(start)) { // some maparea contains start, ok
                    flag = true;
                }
            }
            if !flag { // no maparea contains start, fail.
                return false;
            }
            start += 1;
        }
        return true
    }

    pub fn my_mmap(&mut self, start_va: VirtPageNum, end_va: VirtPageNum, permission: MapPermission) {
        let mut start = start_va.0;
        let mut left = end_va.0;
        let mut right = start_va.0;
        while start < end_va.0 {
            let mut flag = false;
            for i in 0..self.areas.len() {
                let area: &MapArea = &self.areas[i];
                if VirtPageNum::from(area.vpn_range.get_start().0).0 <= start && start < VirtPageNum::from(area.vpn_range.get_end().0).0 { // 包含
                    println!("hahahahahahah {}, start is {}", VirtPageNum::from(area.vpn_range.get_start().0).0, start);
                    // 之前已经判断过，所有frames都不包含start的映射，可以直接添加
                    flag = true;
                    self.areas[i].map_one(&mut self.page_table, VirtPageNum::from(start));
                    println!("malegebi!: {}, i is {}", self.areas[i].data_frames.contains_key(&VirtPageNum::from(65536)), i);
                    println!("map one vpn: {}", start);
                }
            }
            if !flag { // 不包含
                if start < left {
                    left = start;
                }
                if start > right {
                    right = start;
                }
            }
            start += 1;
        }
        right += 1;
        // println!("caonimalegedashabi {}", VirtAddr::from(VirtPageNum::from(left)).0 / 4096);
        self.insert_framed_area(VirtAddr::from(left * 4096), VirtAddr::from(right * 4096), permission);
        // println!("shabishabi: {}",  area.data_frames.len());
        println!("in memory map, add new maparea, left: {}, right: {}", left, right);
    }

    pub fn my_unmap(&mut self, start_va: VirtPageNum, end_va: VirtPageNum) {
        let mut start = start_va.0;
        while start < end_va.0 {
            for i in 0..self.areas.len() {
                if self.areas[i].data_frames.contains_key(&VirtPageNum::from(start)) {

                    self.areas[i].unmap_one(&mut self.page_table, VirtPageNum::from(start));
                    println!("unmap one vpn: {}", start);
                }
                // let area: &MapArea = &self.areas[i];
            }
            start += 1
        }
        let mut i = 0;
        while i < self.areas.len() {
            // let area: &MapArea = &self.areas[i];
            if self.areas[i].data_frames.is_empty() {
                self.areas.remove(i); // warning!! maybe cause an error.
                i -= 1;
            }
            i += 1;
        }

    }

    pub fn include_framed_area(&mut self, start: VirtPageNum, end: VirtPageNum) -> bool {
        for i in 0..self.areas.len() {  
            let area = &self.areas[i];
            if !(area.vpn_range.get_start() >= end || area.vpn_range.get_end() <= start) { // 相交
                return true;
            }
        }
        return false;
    }
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        // let ret = map_area.map(&mut self.page_table);
        // if let Some(data) = data {
        //     map_area.copy_data(&mut self.page_table, data);
        // }
        // self.areas.push(map_area);
        // ret
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&mut self.page_table, data);
        }
        self.areas.push(map_area);


    }
    /// Mention that trampoline is not collected by areas.
    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr::from(TRAMPOLINE).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
    }
    /// Without kernel stacks.
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map kernel sections
        info!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        info!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        info!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        info!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );
        info!("mapping .text section");
        memory_set.push(
            MapArea::new(
                (stext as usize).into(),
                (etext as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
            None,
        );
        info!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        info!("mapping .data section");
        memory_set.push(
            MapArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        info!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        info!("mapping physical memory");
        memory_set.push(
            MapArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        memory_set
    }
    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// also returns user_sp and entry point.
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        // guard page
        user_stack_bottom += PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        memory_set.push(
            MapArea::new(
                user_stack_bottom.into(),
                user_stack_top.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        // map TrapContext
        memory_set.push(
            MapArea::new(
                TRAP_CONTEXT.into(),
                TRAMPOLINE.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        (
            memory_set,
            user_stack_top,
            elf.header.pt2.entry_point() as usize,
        )
    }
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            satp::write(satp);
            core::arch::asm!("sfence.vma");
        }
    }
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }
}

/// map area structure, controls a contiguous piece of virtual memory
pub struct MapArea {
    vpn_range: VPNRange,
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    map_type: MapType,
    map_perm: MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let mut pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        // if vpn.0 == 65536 {

        //     pte_flags |= PTEFlags::U;
        // }
        page_table.map(vpn, ppn, pte_flags);
    }
    #[allow(unused)]
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) -> bool {
        #[allow(clippy::single_match)]
        match self.map_type {
            MapType::Framed => {
                self.data_frames.remove(&vpn);
            }
            _ => {}
        }
        page_table.unmap(vpn)
    }
    pub fn map(&mut self, page_table: &mut PageTable) {
        // let mut vpn = self.vpn_range.get_start().0;
        // let vpn_end = self.vpn_range.get_end().0;
        // while vpn <= vpn_end {
        // println!("self vpn range: {} ,end: {}", self.vpn_range.get_start().0, self.vpn_range.get_end().0);
        for vpn in self.vpn_range {
            // println!("in traverse, vpn is: {}", vpn);
            self.map_one(page_table, VirtPageNum::from(vpn));
            // vpn += 1;
        }

        // println!("self.data_fram len: {}, vpn range:", self.data_frames.len());
    }
    #[allow(unused)]
    pub fn unmap(&mut self, page_table: &mut PageTable) -> bool {
        let mut ret = true;
        for vpn in self.vpn_range {
            ret &= self.unmap_one(page_table, vpn);
        }
        return ret;
    }
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
/// map type for memory set: identical or framed
pub enum MapType {
    Identical,
    Framed,
}

bitflags! {
    /// map permission corresponding to that in pte: `R W X U`
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}
impl MapPermission{
    pub fn clear(&mut self) {
        self.bits = 0;
    }
}

#[allow(unused)]
pub fn remap_test() {
    let mut kernel_space = KERNEL_SPACE.lock();
    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    assert!(!kernel_space
        .page_table
        .translate(mid_text.floor())
        .unwrap()
        .writable());
    assert!(!kernel_space
        .page_table
        .translate(mid_rodata.floor())
        .unwrap()
        .writable());
    assert!(!kernel_space
        .page_table
        .translate(mid_data.floor())
        .unwrap()
        .executable());
    info!("remap_test passed!");
}
