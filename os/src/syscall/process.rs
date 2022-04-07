//! Process management syscalls

use crate::mm::my_map;
use crate::config::{MAX_SYSCALL_NUM, PAGE_SIZE};
use crate::task::{get_cur_task_systimes, TaskStatus , get_cur_task_first_invoked_time, exit_current_and_run_next, suspend_current_and_run_next, current_user_token};
use crate::timer::get_time_us;
use crate::mm::*;
#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    info!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

// YOUR JOB: 引入虚地址后重写 sys_get_time
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let _us = get_time_us();
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let start_va = VirtAddr::from(_ts as usize);
    let vpn = start_va.floor();
    let ppn = page_table.translate(vpn).unwrap().ppn().0;
    let time_ptr = ppn << 12 | start_va.page_offset(); // ppn左移12位拼上offset
    unsafe {
        *(time_ptr as *mut TimeVal) = TimeVal {
            sec: _us / 1_000_000,
            usec: _us % 1_000_000,
        };
    }
    0
}

// CLUE: 从 ch4 开始不再对调度算法进行测试~
pub fn sys_set_priority(_prio: isize) -> isize {
    -1
}

// YOUR JOB: 扩展内核以实现 sys_mmap 和 sys_munmap
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    if _len == 0 {
        return 0;
    }
    if _start % PAGE_SIZE != 0 {// the address hasn't been aligned
        return -1;
    }
    if _port & !0x7 != 0 || _port & 0x7 == 0 { // the port was illegal
        return -1;
    }
    let token = current_user_token();
    let mut flag = 0;
    while flag < _len { // 若len不对齐，则多映射一部分，保证映射以页为单位
        let cur_addr = _start + flag;
        let start_va = VirtAddr::from(cur_addr as usize);
        let vpn = start_va.floor(); // get the vpn
        if let Some(ft) = frame_alloc() {
            let ppn = ft.ppn; // alloc ppn
            let pte_flags = PTEFlags::from_bits(_port as u8).unwrap();
            let succ = my_map(vpn, ppn, pte_flags, token);
            if !succ {
                return -1;
            }
        }
        else { // no physical page available
            return -1;
        }
        flag += PAGE_SIZE;
    }

    0
}

pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    if _start % PAGE_SIZE != 0 {// the address hasn't been aligned
        return -1;
    }
    let token = current_user_token();
    let mut flag = 0;
    while flag < _len { // 若len不对齐，则多映射一部分，保证映射以页为单位
        let cur_addr = _start + flag;
        let start_va = VirtAddr::from(cur_addr as usize);
        let vpn = start_va.floor(); // get the vpn
        let succ = my_munmap(vpn, token);
        if !succ {
            return -1;
        }
        flag += PAGE_SIZE;
    }
    0
}

// YOUR JOB: 引入虚地址后重写 sys_task_info
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let start_va = VirtAddr::from(ti as usize);
    let vpn = start_va.floor();
    let ppn = page_table.translate(vpn).unwrap().ppn().0;
    let time_ptr = ppn << 12 | start_va.page_offset() as usize; // ppn左移12位拼上offset
    let arr = get_cur_task_systimes();
    unsafe{
        for i in 0..MAX_SYSCALL_NUM{
            (*(time_ptr as *mut TaskInfo)).syscall_times[i] = arr[i];
            
        }
        (*(time_ptr as *mut TaskInfo)).status = TaskStatus::Running; // TODO:可能需要修改
        // println!("in process: {}",(*(time_ptr as *mut TaskInfo)).status == TaskStatus::Running );
        (*(time_ptr as *mut TaskInfo)).time = (get_time_us() - get_cur_task_first_invoked_time()) / 1000;
    }
    0
}
