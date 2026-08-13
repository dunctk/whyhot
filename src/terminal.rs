use std::{io, mem::MaybeUninit, os::fd::AsRawFd};

pub struct RawTerminal {
    original: libc::termios,
}

impl RawTerminal {
    pub fn enter() -> io::Result<Self> {
        let fd = io::stdin().as_raw_fd();
        let mut original = MaybeUninit::<libc::termios>::uninit();
        if unsafe { libc::tcgetattr(fd, original.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let original = unsafe { original.assume_init() };
        let mut raw = original;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 0;
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }
        print!("\x1b[?1049h\x1b[?25l");
        Ok(Self { original })
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        unsafe { libc::tcsetattr(io::stdin().as_raw_fd(), libc::TCSANOW, &self.original) };
        print!("\x1b[?25h\x1b[?1049l");
    }
}

pub fn size() -> (usize, usize) {
    let mut value = MaybeUninit::<libc::winsize>::zeroed();
    let result = unsafe {
        libc::ioctl(
            io::stdout().as_raw_fd(),
            libc::TIOCGWINSZ,
            value.as_mut_ptr(),
        )
    };
    if result == 0 {
        let value = unsafe { value.assume_init() };
        (value.ws_col.max(40) as usize, value.ws_row.max(12) as usize)
    } else {
        (80, 24)
    }
}
