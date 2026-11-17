//! CLAP POSIX descriptors are serviced only on the application main thread.
use super::*;
use daw_plugin_clap::sys::ext::posix_fd_support::*;
unsafe extern "C" fn register(host: *const clap_host, fd: i32, flags: u32) -> bool {
    if fd < 0 || flags == 0 || flags & !7 != 0 || !unsafe { is_main(host) } {
        return false;
    }
    let mut descriptors = unsafe { data(host) }.fds.borrow_mut();
    if descriptors.len() == 128 || descriptors.iter().any(|(old, _)| *old == fd) {
        return false;
    }
    descriptors.push((fd, flags));
    true
}
unsafe extern "C" fn modify(host: *const clap_host, fd: i32, flags: u32) -> bool {
    if flags == 0 || flags & !7 != 0 || !unsafe { is_main(host) } {
        return false;
    }
    let mut descriptors = unsafe { data(host) }.fds.borrow_mut();
    let Some((_, old)) = descriptors.iter_mut().find(|(old, _)| *old == fd) else {
        return false;
    };
    *old = flags;
    true
}
unsafe extern "C" fn unregister(host: *const clap_host, fd: i32) -> bool {
    if !unsafe { is_main(host) } {
        return false;
    }
    let mut descriptors = unsafe { data(host) }.fds.borrow_mut();
    let Some(index) = descriptors.iter().position(|(old, _)| *old == fd) else {
        return false;
    };
    descriptors.swap_remove(index);
    true
}
pub(super) static HOST: clap_host_posix_fd_support = clap_host_posix_fd_support {
    register_fd: Some(register),
    modify_fd: Some(modify),
    unregister_fd: Some(unregister),
};
pub(super) fn pump(instance: &ClapInstance) {
    let extension =
        unsafe { instance.ext::<clap_plugin_posix_fd_support>(CLAP_EXT_POSIX_FD_SUPPORT) };
    if extension.is_null() {
        return;
    }
    let Some(callback) = (unsafe { (*extension).on_fd }) else {
        return;
    };
    let mut descriptors = [super::super::linux::PollFd {
        fd: -1,
        events: 0,
        revents: 0,
    }; 128];
    let count;
    {
        let registered = instance.data.fds.borrow();
        count = registered.len();
        for (out, (fd, flags)) in descriptors.iter_mut().zip(registered.iter()) {
            out.fd = *fd;
            out.events = if flags & CLAP_POSIX_FD_READ != 0 {
                1
            } else {
                0
            } | if flags & CLAP_POSIX_FD_WRITE != 0 {
                4
            } else {
                0
            };
        }
    }
    if !super::super::linux::ready(&mut descriptors[..count]) {
        return;
    }
    for descriptor in &descriptors[..count] {
        let mut flags = 0;
        if descriptor.revents & 1 != 0 {
            flags |= CLAP_POSIX_FD_READ;
        }
        if descriptor.revents & 4 != 0 {
            flags |= CLAP_POSIX_FD_WRITE;
        }
        if descriptor.revents & (8 | 16 | 32) != 0 {
            flags |= CLAP_POSIX_FD_ERROR;
        }
        if flags != 0
            && instance
                .data
                .fds
                .borrow()
                .iter()
                .any(|(fd, _)| *fd == descriptor.fd)
        {
            unsafe {
                callback(instance.plugin, descriptor.fd, flags);
            }
        }
    }
}
