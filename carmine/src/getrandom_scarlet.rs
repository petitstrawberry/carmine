use scarlet_sys::{syscall3, Syscall};

const SYSCALL_ERROR: usize = usize::MAX;

getrandom::register_custom_getrandom!(scarlet_getrandom);

fn scarlet_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    let ret = syscall3(Syscall::GetRandom, buf.as_mut_ptr() as usize, buf.len(), 0);
    if ret == SYSCALL_ERROR || ret > buf.len() {
        Err(getrandom::Error::UNSUPPORTED)
    } else {
        Ok(())
    }
}
