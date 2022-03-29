mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::config::{MAX_APP_NUM, MAX_SYSCALL_NUM};
use crate::loader::{get_num_app, init_app_cx};
use crate::sync::UPSafeCell;
use crate::timer::{get_time, get_time_us};
use lazy_static::*;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM],
    current_task: usize,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_cx: TaskContext::zero_init(),
            task_status: TaskStatus::UnInit,
            syscall_times:[0; MAX_SYSCALL_NUM],
            task_first_invoked_time: 0,
            task_end_time: 0,
        }; MAX_APP_NUM];
        for (i, t) in tasks.iter_mut().enumerate().take(num_app) {
            t.task_cx = TaskContext::goto_restore(init_app_cx(i));
            t.task_status = TaskStatus::Ready;
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
    fn mark_task_first_invoked_time(&self, task: &mut TaskControlBlock) {
        if(task.task_first_invoked_time == 0) { // 不是0, 说明已经标记过第一次调用的时间
            task.task_first_invoked_time = get_time();
        }
    }
    fn mark_task_end_time(&self, task: &mut TaskControlBlock) {
        if(task.task_end_time == 0) { 
            task.task_end_time = get_time();
        }
    }
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        self.mark_task_first_invoked_time(task0);
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }


    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            self.mark_task_first_invoked_time(&mut inner.tasks[next]);
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


    fn add_curtask_systime(&self, syscall_id: usize) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        // println!("{}", inner.tasks[current].syscall_times[syscall_id]);
        inner.tasks[current].syscall_times[syscall_id] += 1;
        // println!("{}", inner.tasks[current].syscall_times[syscall_id]);

    }

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

    fn get_cur_task_end_time(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        if inner.tasks[current].task_end_time == 0 {
            get_time()
        }
        else {
            inner.tasks[current].task_end_time
        }
    }
}


pub fn get_cur_task_end_time() -> usize {
    TASK_MANAGER.get_cur_task_end_time()
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

pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}


pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}
