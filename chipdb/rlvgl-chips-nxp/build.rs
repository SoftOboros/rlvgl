/*!
Build script for rlvgl-chips-nxp.

This placeholder script tracks changes to the RLVGL_CHIP_SRC
environment variable and will embed extracted board definitions
in future iterations.
*/
fn main() {
    println!("cargo:rerun-if-env-changed=RLVGL_CHIP_SRC");
}
