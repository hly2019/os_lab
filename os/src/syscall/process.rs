//! Process management syscalls

use crate::mm::{translated_refmut, translated_ref, translated_str, PageTable, VirtAddr, MapPermission, VirtPageNum};
use crate::task::{
    add_task, current_task, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next, TaskStatus,judge_map_right, my_mmap, judge_unmap_right, my_umap,
    get_cur_task_systimes, get_cur_task_first_invoked_time,
};
use crate::fs::{open_file, OpenFlags};
use crate::timer::get_time_us;
use alloc::sync::Arc;
use alloc::vec::Vec;
use crate::config:: {MAX_SYSCALL_NUM, PAGE_SIZE};
use alloc::string::String;

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
    debug!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

/// Syscall Fork which returns 0 for child process and child_pid for parent process
pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}




/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();
    // find a child process

    // ---- access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB lock exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after removing from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child TCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB lock automatically
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

// YOUR JOB: 实现sys_set_priority，为任务添加优先级
pub fn sys_set_priority(_prio: isize) -> isize {
    let current_task = current_task().unwrap();
    let mut ret = -1;
    if _prio >= 2 { 
        ret = _prio; 
    }
    current_task.set_priority(_prio);
    ret
}

// YOUR JOB: 扩展内核以实现 sys_mmap 和 sys_munmap
pub fn sys_mmap(_start: usize, _len: usize, _prot: usize) -> isize {
    if _len == 0 {
        return 0;
    }
    if _start % PAGE_SIZE != 0{// the address hasn't been aligned
        return -1;
    }
    if _prot & !0x7 != 0 || _prot & 0x7 == 0 { // the port was illegal
        return -1;
    }

    let vpn_start = VirtAddr::from(_start);
    let vpn_end = VirtAddr::from(_start + _len);
    let mut permission = MapPermission::U;
    if _prot & 1 != 0 { // readable
        permission |= MapPermission::R;
    }
    if _prot & 2 != 0 { // writable
        permission |= MapPermission::W;
    }
    if _prot & 4 != 0 {
        permission |= MapPermission::X;
    }
    let ceil = VirtPageNum::from(vpn_end.ceil().0);
    println!("in mmap, start: {}, end: {}", VirtPageNum::from(vpn_start.floor().0).0, VirtPageNum::from(vpn_end.ceil().0).0);

    if !judge_map_right(vpn_start.floor(), ceil) {
        return -1;
    }
    my_mmap(vpn_start.floor(), ceil, permission);
    0
}

pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    if _start % PAGE_SIZE != 0 {// the address hasn't been aligned
        return -1;
    }
    let vpn_start = VirtAddr::from(_start);
    let vpn_end = VirtAddr::from(_start + _len);
    let ceil = VirtPageNum::from(vpn_end.ceil().0);
    if !judge_unmap_right(vpn_start.floor(), ceil) {
        return -1;
    }
    my_umap(vpn_start.floor(), ceil);

    
    0
}

//
// YOUR JOB: 实现 sys_spawn 系统调用
// ALERT: 注意在实现 SPAWN 时不需要复制父进程地址空间，SPAWN != FORK + EXEC 
pub fn sys_spawn(_path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let new_task = current_task().unwrap().spawn(all_data.as_slice());
        let new_pid = new_task.pid.0;
        let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
        trap_cx.x[10] = 0;

        add_task(new_task);
        return new_pid as isize;
    }
    -1
}

/// Syscall Exec which accepts the elf path
pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}
