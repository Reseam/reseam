use super::WasmState;
use super::stitch::patch::resources::Host;
use super::stitch::patch::types::ResourceRef;

impl Host for WasmState {
    fn has_resources(&mut self) -> bool {
        self.ctx().resources().is_some()
    }

    fn get_string(&mut self, index: u32) -> Option<String> {
        self.ctx().resources().and_then(|r| r.get_string(index).map(|s| s.to_string()))
    }

    fn set_string(&mut self, index: u32, value: String) {
        if let Some(res) = self.ctx().resources_mut() {
            res.set_string(index, value);
        }
    }

    fn resource_id(&mut self, res_type: String, res_name: String) -> Option<i64> {
        self.ctx().find_resource_id(&res_type, &res_name).map(|id| id as i64)
    }

    fn find_entries_by_string(&mut self, string_index: u32) -> Vec<ResourceRef> {
        let ctx = self.ctx();
        let res = match ctx.resources() {
            Some(r) => r,
            None => return Vec::new(),
        };
        res.find_entries_by_string(string_index)
            .into_iter()
            .map(|e| ResourceRef {
                res_id: e.res_id,
                package_id: e.package_id,
                type_id: e.type_id,
                entry_index: e.entry_index,
                key_name: e.key_name,
            })
            .collect()
    }

    fn replace_entry_string(&mut self, res_id: u32, new_string_index: u32) {
        if let Some(res) = self.ctx().resources_mut() {
            res.replace_entry_string(res_id, new_string_index);
        }
    }

    fn copy_file(&mut self, bundle_path: String, apk_path: String) {
        let full_path = match &self.bundle_dir {
            Some(dir) => dir.join(&bundle_path),
            None => std::path::PathBuf::from(&bundle_path),
        };
        if let Ok(data) = std::fs::read(&full_path) {
            self.ctx().inject_file(&apk_path, data);
        }
    }

    fn copy_resource_group(&mut self, res_type: String, files: Vec<String>) {
        let bundle_dir = match &self.bundle_dir {
            Some(dir) => dir.clone(),
            None => return,
        };
        for file_name in &files {
            let src = bundle_dir.join("resources").join(&res_type).join(file_name);
            if let Ok(data) = std::fs::read(&src) {
                let apk_path = format!("res/{res_type}/{file_name}");
                self.ctx().inject_file(&apk_path, data);
            }
        }
    }

    fn delete_file(&mut self, apk_path: String) {
        self.ctx().delete_file(&apk_path);
    }

    fn list_files(&mut self, prefix: String) -> Vec<String> {
        self.ctx().list_files().iter()
            .filter(|f| f.starts_with(&prefix))
            .cloned()
            .collect()
    }
}
