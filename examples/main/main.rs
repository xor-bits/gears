extern crate gears;

use gears::{GearsBuilder, VSync};
#[cfg(target_arch = "wasm32")]
use log::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    main();
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    #[cfg(target_arch = "wasm32")]
    wasm_logger::init(
        wasm_logger::Config::new(Level::Debug), /* .module_prefix("main")
                                                .module_prefix("gears::renderer") */
    );
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    let gears = GearsBuilder::new().with_vsync(VSync::Off).build();
    gears.run();
}
