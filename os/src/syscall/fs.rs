//! File and filesystem-related syscalls

use crate::mm::translated_byte_buffer;
use crate::task::current_user_token;
use crate::config::{PAGE_SIZE};
use crate::mm::*;
const FD_STDOUT: usize = 1;

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let mut flag = 0;
    let mut temp_buf = buf as usize;
    let mut illegal = 0;
    if buf as usize % PAGE_SIZE != 0 {
        temp_buf = (((buf as usize) / PAGE_SIZE) as usize) * PAGE_SIZE;
        println!("buf: {}, temp_buf: {}", buf as usize, temp_buf);
    }
    while flag <= len { // 为啥改成<=就对了。。。
        let cur_addr = temp_buf as usize + flag;
        let start_va = VirtAddr::from(cur_addr as usize);
        let vpn = start_va.floor(); // get the vpn
        let mut page_table = PageTable::from_token(token);
        let pte = page_table.find_pte_create(vpn).unwrap();
        let exe = pte.executable();
        if !pte.writable() || !pte.is_valid() || !pte.PTE_U(){
            // illegal = 1;
            // println!("illegal");
            return -1;
            // break;
        }
        flag += PAGE_SIZE;
    }
    // return -1;
    match fd {
        FD_STDOUT => {
            let buffers = translated_byte_buffer(current_user_token(), buf, len);
            for buffer in buffers {
                print!("{}", core::str::from_utf8(buffer).unwrap());
            }
            len as isize
        }
        _ => {
            panic!("Unsupported fd in sys_write!");
        }
    }
}
