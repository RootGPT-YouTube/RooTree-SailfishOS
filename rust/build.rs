// Il ponte verso il C++ lo genera cxx-build; l'intestazione si copia in
// target/cxxbridge/, un percorso fisso che il CMake dell'app conosce
// (quello vero sta in OUT_DIR, che cambia a ogni hash di build).
use std::{env, fs, path::PathBuf};

fn main() {
    cxx_build::bridge("src/lib.rs").std("c++17").compile("rootree-core-bridge");
    println!("cargo:rerun-if-changed=src/lib.rs");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let include = out.join("cxxbridge/include");
    // OUT_DIR = target/<triple>/<profilo>/build/<pkg>-<hash>/out
    let dest = out.ancestors().nth(3).unwrap().join("cxxbridge");
    fs::create_dir_all(dest.join("rust")).unwrap();
    fs::copy(include.join("rust/cxx.h"), dest.join("rust/cxx.h")).unwrap();
    fs::copy(include.join("rootree-core/src/lib.rs.h"), dest.join("rootree_core.h")).unwrap();
}
