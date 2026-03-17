use super::WasmState;
use super::stitch::patch::log::Host;

impl Host for WasmState {
    fn info(&mut self, msg: String) {
        self.ctx().log().info(&msg);
    }

    fn warn(&mut self, msg: String) {
        self.ctx().log().warn(&msg);
    }

    fn debug(&mut self, msg: String) {
        self.ctx().log().debug(&msg);
    }
}
