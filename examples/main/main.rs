use gears::Gears;
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
    console_log::init_with_level(Level::Debug).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    let gears = Gears::new(900, 700);
    gears.run();
}
