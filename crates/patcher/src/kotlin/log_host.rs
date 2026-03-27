use boltffi::export;

use super::with_ctx;

#[export]
pub fn log_info(msg: String) {
    with_ctx(|ctx| ctx.log().info(msg));
}

#[export]
pub fn log_warn(msg: String) {
    with_ctx(|ctx| ctx.log().warn(msg));
}

#[export]
pub fn log_debug(msg: String) {
    with_ctx(|ctx| ctx.log().debug(msg));
}
