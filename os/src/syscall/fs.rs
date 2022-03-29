const FD_STDOUT: usize = 1;
use crate::task::get_cur_task;
use crate::loader::judge_ptr_in_range;
// YOUR JOB: 修改 sys_write 使之通过测试
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let cur = get_cur_task();
    let flag = judge_ptr_in_range(cur, buf, len);
    match fd + flag as usize {
        FD_STDOUT => {
            let slice = unsafe { core::slice::from_raw_parts(buf, len) };
            let str = core::str::from_utf8(slice).unwrap();
            print!("{}", str);
            len as isize
        }
        _ => {
            panic!("Unsupported fd in sys_write!");
        }
    }
}
