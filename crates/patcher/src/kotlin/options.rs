use boltffi::export;

use super::with_ctx;

#[export]
pub fn option_get_string(key: String) -> Option<String> {
    with_ctx(|ctx| ctx.options().get_string(&key).map(|s| s.to_string()))
}

#[export]
pub fn option_get_bool(key: String) -> Option<bool> {
    with_ctx(|ctx| ctx.options().get_bool(&key))
}

#[export]
pub fn option_get_int(key: String) -> Option<i64> {
    with_ctx(|ctx| ctx.options().get_int(&key))
}

#[export]
pub fn option_get_float(key: String) -> Option<f64> {
    with_ctx(|ctx| ctx.options().get_float(&key))
}

#[export]
pub fn option_get_string_list(key: String) -> Option<Vec<String>> {
    with_ctx(|ctx| ctx.options().get_string_list(&key).map(|sl| sl.to_vec()))
}

#[export]
pub fn option_get_path(key: String) -> Option<String> {
    with_ctx(|ctx| {
        ctx.options()
            .get_path(&key)
            .map(|path| path.to_string_lossy().into_owned())
    })
}

#[export]
pub fn option_list_path_contents(key: String) -> Option<Vec<String>> {
    with_ctx(|ctx| ctx.options().list_path_contents(&key).ok().flatten())
}

#[export]
pub fn option_read_path_file(key: String, relative_path: String) -> Option<Vec<u8>> {
    with_ctx(|ctx| {
        ctx.options()
            .read_path_file(&key, &relative_path)
            .ok()
            .flatten()
    })
}
