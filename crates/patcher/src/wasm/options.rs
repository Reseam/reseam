use super::WasmState;
use super::stitch::patch::options::Host;

impl Host for WasmState {
    fn get_string(&mut self, key: String) -> Option<String> {
        self.ctx().options().get_string(&key).map(|s| s.to_string())
    }

    fn get_bool(&mut self, key: String) -> Option<bool> {
        self.ctx().options().get_bool(&key)
    }

    fn get_int(&mut self, key: String) -> Option<i64> {
        self.ctx().options().get_int(&key)
    }

    fn get_float(&mut self, key: String) -> Option<f64> {
        self.ctx().options().get_float(&key)
    }

    fn get_string_list(&mut self, key: String) -> Option<Vec<String>> {
        self.ctx().options().get_string_list(&key).map(|sl| sl.to_vec())
    }

    fn list_path_contents(&mut self, key: String) -> Option<Vec<String>> {
        self.ctx().options().list_path_contents(&key).ok().flatten()
    }

    fn read_path_file(&mut self, key: String, relative_path: String) -> Option<Vec<u8>> {
        self.ctx().options().read_path_file(&key, &relative_path).ok().flatten()
    }
}
