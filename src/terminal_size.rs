// This file is c/p from termion and then modified to take an arbitrary FD
// https://docs.rs/termion/3.0.0/src/termion/sys/unix/size.rs.html

use std::os::fd::AsRawFd;

use libc::{c_ushort, ioctl, TIOCGWINSZ};

#[repr(C)]
struct TermSize {
    row: c_ushort,
    col: c_ushort,
    x: c_ushort,
    y: c_ushort,
}

/// Get the size (columns, rows) of the terminal.
pub fn terminal_size(fd: &impl AsRawFd) -> std::io::Result<(u16, u16)> {
    unsafe {
        let mut size: TermSize = std::mem::zeroed();
        let result = ioctl(fd.as_raw_fd(), TIOCGWINSZ, std::ptr::addr_of_mut!(size));
        if result == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok((size.col, size.row))
        }
    }
}
