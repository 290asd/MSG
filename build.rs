// Gives the Windows exe its icon (img/msg_icon.ico, drawn by img/logo.py).
fn main() {
    println!("cargo:rerun-if-changed=img/msg_icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new().set_icon("img/msg_icon.ico").compile().expect("the exe's icon");
    }
}
