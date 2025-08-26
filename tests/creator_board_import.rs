//! Tests converting user `.ioc` files into board overlays via creator helpers.

#[path = "../src/bin/creator/bsp/af.rs"]
pub mod af;
#[path = "../src/bin/creator/bsp/ioc.rs"]
pub mod ioc;
#[path = "../src/bin/creator/bsp/ir.rs"]
pub mod ir;
mod bsp {
    pub use super::af;
    pub use super::ioc;
}
#[path = "../src/bin/creator/board_import.rs"]
mod board_import;

use std::path::Path;
use tempfile::tempdir;

#[test]
fn imports_custom_ioc() {
    let data_dir = Path::new("tests/data/tools_st_extract_af");
    let ioc = data_dir.join("sample.ioc");
    let tmp = tempdir().unwrap();
    let out = tmp.path().join("board.json");
    board_import::from_ioc(&ioc, "MyBoard", &out, None).expect("convert");
    let text = std::fs::read_to_string(&out).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["board"], "MyBoard");
    assert_eq!(json["chip"], "STM32F4");
    assert_eq!(json["pins"]["PA0"]["USART2_TX"], 7);
    assert_eq!(json["pins"]["PA1"]["USART2_RX"], 7);
}
