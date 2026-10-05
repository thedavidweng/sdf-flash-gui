//! wasm32 entry point for the browser demo (ADR 0011).
//!
//! Mounts [`sdf_flash_gui::gui::create_web_demo`] on the `<canvas>` whose id is
//! passed from JavaScript. All UI comes from the desktop crate.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
pub async fn start(canvas_id: String) -> Result<(), JsValue> {
    let canvas = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(&canvas_id))
        .ok_or_else(|| JsValue::from_str(&format!("missing #{canvas_id}")))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|cc| Ok(sdf_flash_gui::gui::create_web_demo(cc))),
        )
        .await
}
