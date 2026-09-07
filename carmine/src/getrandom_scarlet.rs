use scarlet_sys::{Syscall, syscall3};

const SYSCALL_ERROR: usize = usize::MAX;

getrandom::register_custom_getrandom!(scarlet_getrandom);

fn scarlet_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    // SAFETY: buf is exclusively borrowed and writable for the full synchronous call.
    let ret = unsafe { syscall3(Syscall::GetRandom, buf.as_mut_ptr() as usize, buf.len(), 0) };
    if ret == SYSCALL_ERROR || ret > buf.len() {
        Err(getrandom::Error::UNSUPPORTED)
    } else {
        Ok(())
    }
}
