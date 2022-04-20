//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see [`__switch`]. Control flow around this function
//! might not be what you expect.

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;
use crate::config::{MAX_SYSCALL_NUM};
use crate::timer::get_time_us;
use crate::loader::{get_app_data, get_num_app};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
pub use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};
use crate::mm::*;
pub use context::TaskContext;

/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task state transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner` in
/// existing functions on `TaskManager`.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

/// The task manager inner in 'UPSafeCell'
struct TaskManagerInner {
    /// task list
    tasks: Vec<TaskControlBlock>,
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
    /// a `TaskManager` instance through lazy_static!
    pub static ref TASK_MANAGER: TaskManager = {
        info!("init TASK_MANAGER");
        let num_app = get_num_app();
        info!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// Run the first task in task list.
    ///
    /// Generally, the first task in task list is an idle task (we call it zero process later).
    /// But in ch4, we load apps statically, so the first task is a real app.
    fn mark_task_first_invoked_time(&self, task: &mut TaskControlBlock) {
        if(task.task_first_invoked_time == 0) { // 不是0, 说明已经标记过第一次调用的时间
            task.task_first_invoked_time = get_time_us();
        }
    }
    
    fn get_cur_task(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.current_task
    }

    pub fn my_munmap(&self, start_va: VirtPageNum, end_va: VirtPageNum) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].memory_set.my_unmap(start_va, end_va);
    }
    pub fn my_mmap(&self, start_va: VirtPageNum, end_va: VirtPageNum, permission: MapPermission) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].memory_set.my_mmap(start_va, end_va, permission);
    }


    pub fn judge_unmap_right(&self, start_va: VirtPageNum, end_va: VirtPageNum) -> bool {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].memory_set.judge_unmap_right(start_va, end_va)
    }

    pub fn judge_mmap_right(&self, start_va: VirtPageNum, end_va: VirtPageNum) -> bool {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].memory_set.judge_map_right(start_va, end_va)
    }

    pub fn map(&self, vpn_start: VirtAddr, vpn_end: VirtAddr, permission:MapPermission) -> bool {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        // println!("{}", inner.tasks[current].syscall_times[syscall_id]);
        // inner.tasks[current].memory_set.map_one();
        if inner.tasks[current].memory_set.include_framed_area(vpn_start.floor(), vpn_end.ceil()) {
            return false;
        }
        else {
            return inner.tasks[current].memory_set.insert_framed_area(VirtAddr::from(vpn_start), VirtAddr::from(vpn_end), permission);
            // return true;
        }
    }

    pub fn unmap(&self, vpn_start: VirtAddr, vpn_end: VirtAddr) -> bool {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        // println!("{}", inner.tasks[current].syscall_times[syscall_id]);
        // inner.tasks[current].memory_set.map_one();
        if !inner.tasks[current].memory_set.include_framed_area(vpn_start.floor(), vpn_end.ceil()) {
            // 没人包含，不对
            println!("in unmap case 1");
            return false;
        }
        else {
            println!("in unmap case 2");
            return inner.tasks[current].memory_set.cancel_framed_area(vpn_start, vpn_end);
            // inner.tasks[current].memory_set.insert_framed_area(VirtAddr::from(vpn_start), VirtAddr::from(vpn_end), permission);
            // return true;
        }
    }


    fn add_curtask_systime(&self, syscall_id: usize) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        // println!("{}", inner.tasks[current].syscall_times[syscall_id]);
        inner.tasks[current].syscall_times[syscall_id] += 1;
        // println!("{}", inner.tasks[current].syscall_times[syscall_id]);

    }

    // fn get_cur_task_mem_set(&self) -> MemorySet {
    //     let mut inner = self.inner.exclusive_access();
    //     let current = inner.current_task;
    //     // println!("{}", inner.tasks[current].syscall_times[syscall_id]);
    //     inner.tasks[current].memory_set
        
    // }

    fn get_cur_task_systimes(&self) ->[u32; MAX_SYSCALL_NUM] {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        // println!("{}", inner.tasks[current].syscall_times);
        inner.tasks[current].syscall_times
    }

    fn get_cur_task_first_invoked_time(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_first_invoked_time
    }





    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        self.mark_task_first_invoked_time(next_task);
        next_task.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut _, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// Change the status of current `Running` task into `Ready`.
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    /// Change the status of current `Running` task into `Exited`.
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    /// Find next task to run and return task id.
    ///
    /// In this case, we only return the first `Ready` task in task list.
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// Get the current 'Running' task's token.
    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_user_token()
    }

    #[allow(clippy::mut_from_ref)]
    /// Get the current 'Running' task's trap contexts.
    fn get_current_trap_cx(&self) -> &mut TrapContext {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_trap_cx()
    }

    /// Switch current `Running` task to the task we have found,
    /// or there is no `Ready` task and we can exit with all applications completed
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            self.mark_task_first_invoked_time(&mut inner.tasks[next]);
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }
}

pub fn get_cur_task() -> usize {
    TASK_MANAGER.get_cur_task()
}

// map(&self, vpn_start: VirtAddr, vpn_end: VirtAddr, permission:MapPermission)
pub fn used_map(vpn_start: VirtAddr, vpn_end: VirtAddr, permission: MapPermission) -> bool {
    TASK_MANAGER.map(vpn_start, vpn_end, permission)
}

pub fn my_mmap(vpn_start: VirtPageNum, vpn_end: VirtPageNum, permission: MapPermission) {
    TASK_MANAGER.my_mmap(vpn_start, vpn_end, permission);
}
pub fn my_umap(vpn_start: VirtPageNum, vpn_end: VirtPageNum) {
    TASK_MANAGER.my_munmap(vpn_start, vpn_end);
}

pub fn judge_unmap_right(start_va: VirtPageNum, end_va: VirtPageNum) -> bool {
    TASK_MANAGER.judge_unmap_right(start_va, end_va)
}

pub fn judge_map_right(start_va: VirtPageNum, end_va: VirtPageNum) -> bool {
    TASK_MANAGER.judge_mmap_right(start_va, end_va)
}


pub fn used_unmap(vpn_start: VirtAddr, vpn_end: VirtAddr)-> bool {
    TASK_MANAGER.unmap(vpn_start, vpn_end)
}
pub fn get_cur_task_first_invoked_time() -> usize {
    TASK_MANAGER.get_cur_task_first_invoked_time()
}

pub fn add_curtask_systimes(id: usize) {
    TASK_MANAGER.add_curtask_systime(id);
}

pub fn get_cur_task_systimes() ->[u32; MAX_SYSCALL_NUM] {
    TASK_MANAGER.get_cur_task_systimes()
}


/// Run the first task in task list.
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// Switch current `Running` task to the task we have found,
/// or there is no `Ready` task and we can exit with all applications completed
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// Change the status of current `Running` task into `Ready`.
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// Change the status of current `Running` task into `Exited`.
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

/// Get the current 'Running' task's token.
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

/// Get the current 'Running' task's trap contexts.
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}
