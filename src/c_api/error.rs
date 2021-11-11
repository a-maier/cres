use anyhow::{anyhow, Error};

use std::cell::RefCell;
use std::os::raw::c_char;

thread_local!{
    pub(crate) static LAST_ERROR: RefCell<Option<Error>> = RefCell::new(None);
}

#[no_mangle]
pub extern "C" fn cres_print_last_err() {
    let _ = std::panic::catch_unwind(
        || LAST_ERROR.with(
            |e| if let Some(err) = &*e.borrow() {
                eprintln!("{}", err);
            }
        )
    );
}

#[no_mangle]
#[must_use]
pub extern "C" fn cres_get_last_err(buf: * mut c_char, buflen: usize) -> i32 {
    match std::panic::catch_unwind(
        || cres_last_err_internal(buf, buflen)
    ) {
        Ok(i) => i,
        Err(err) => {
            LAST_ERROR.with(|e| *e.borrow_mut() = Some(anyhow!("panic: {:?}", err)));
            -1
        }
    }
}

fn cres_last_err_internal(buf: * mut c_char, buflen: usize) -> i32 {
    let err = LAST_ERROR.with(
        |e| {
            let e: &Option<_> = &e.borrow();
            e.as_ref().map(|e| e.to_string())
        }
    );
    if let Some(msg) = err {
        let msg_len = msg.as_bytes().len();
        if buflen == 0 {
            return (1 + msg_len) as i32;
        }
        let len = std::cmp::min(msg_len, (buflen as usize) - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(
                msg.as_bytes().as_ptr() as * const c_char,
                buf,
                len
            );
            *buf.offset(1 + len as isize) = 0;
        }
        if len < msg_len {
            (1 + msg_len) as i32
        } else {
            0
        }
    } else {
        0
    }
}
