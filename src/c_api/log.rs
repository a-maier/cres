use crate::c_api::error::LAST_ERROR;

use std::ffi::{CStr, OsStr};
use std::os::raw::c_char;
use std::os::unix::ffi::OsStrExt;

use anyhow::anyhow;
use env_logger::Env;

#[no_mangle]
#[must_use]
pub extern "C" fn cres_logger_from_env(env_var: * const c_char) -> i32 {
    let res = std::panic::catch_unwind(
        || {
            let env_var = unsafe{
                CStr::from_ptr(env_var)
            };
            let env_var = OsStr::from_bytes(env_var.to_bytes());
            let env_var = env_var.to_str().unwrap();

            let env = Env::default().filter_or(env_var, "info");

            env_logger::init_from_env(env);
        }
    );

    if let Err(err) = res {
        LAST_ERROR.with(
            |e| *e.borrow_mut() = Some(anyhow!("panic: {:?}", err))
        );
        -1
    } else {
        0
    }
}
