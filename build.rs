#![deny(rust_2018_idioms)]

#[cfg(all(windows, target_env = "msvc"))]
use std::path::PathBuf;
use std::{env, path::Path};

#[cfg(any(unix, target_env = "gnu"))]
use std::process::Command;

/// Outputs the library-file's prefix as word usable for actual arguments on
/// commands or paths.
fn rustc_linking_word(is_static_link: bool) -> &'static str {
    if is_static_link {
        "static"
    } else {
        "dylib"
    }
}

/// Builds Opus on Unix or GNU.
/// If we want to build for Window's GNU-toolchain, we need to build in MSYS2.
///
/// Building Opus consists of four steps:
/// 1. Run `autogen.sh`.
/// 2. Configure the generated file to prepare building.
/// 3. Building Opus.
/// 4. Installing the built Opus in `OUT_DIR`.
#[cfg(any(unix, target_env = "gnu"))]
fn build_opus(build_directory: &Path, is_static: bool, installed_lib_directory: &Option<String>) {
    let is_static_text = rustc_linking_word(is_static);

    if let Some(prebuilt_directory) = installed_lib_directory {
        println!(
            "{}",
            format!("cargo:rustc-link-lib={}=opus", is_static_text)
        );
        println!("cargo:rustc-link-search=native={}", prebuilt_directory);

        return;
    }

    let opus_path = Path::new("opus")
        .canonicalize()
        .expect("Could not canonicalise.");

    println!(
        "cargo:info=Opus source path: {:?}.",
        &opus_path.to_string_lossy()
    );
    println!(
        "cargo:info=Opus will be built as {}-library.",
        is_static_text
    );

    let copy_command_result = Command::new("cp")
        .arg("-r")
        .arg(&opus_path)
        .arg(&build_directory)
        .status()
        .expect(&format!(
            "Failed to copy Opus files to: {}",
            &build_directory
                .to_str()
                .expect("Build Path contains invalid characters.")
        ));

    if !copy_command_result.success() {
        panic!("Failed to copy Opus files.");
    }

    let opus_path = build_directory.join("opus");

    let sh_command_result = Command::new("sh")
        .arg("autogen.sh")
        .current_dir(&opus_path)
        .status()
        .expect("Failed to run `sh autogen.sh`.");

    if !sh_command_result.success() {
        panic!("Failed to autogen Opus.");
    }

    let mut command_builder = Command::new("sh");
    command_builder.arg("configure");

    if is_static {
        command_builder
            .arg("--enable-static")
            .arg("--disable-shared");
    } else {
        command_builder
            .arg("--disable-static")
            .arg("--enable-shared");
    }

    if is_cross_compiled() {
        println!("cargo:info=Opus will be built for cross-compilation.");
        command_builder
            .arg("--host");
    }

    if is_target_x32() {
        println!("cargo:info=Opus will be built for 32-bit.");

        command_builder
            .env("LDFLAGS", "-g -O2 -m32")
            .env("CFLAGS", "-g -O2 -m32");
    } else {
        println!("cargo:info=Opus will be built for 64-bit.");
    }

    let command_result = command_builder
        .arg("--disable-doc")
        .arg("--disable-extra-programs")
        .arg("--with-pic")
        .arg("--prefix")
        .arg(
            build_directory
                .to_str()
                .expect("Build Path contains invalid characters.")
                .replace("\\", "/"),
        )
        .current_dir(&opus_path)
        .status()
        .expect("Failed to run `configure` Opus.");

    if !command_result.success() {
        panic!("Failed to configure Opus.");
    }

    let make_command_result = Command::new("make")
        .current_dir(&opus_path)
        .status()
        .expect("Failed to run `make`.");

    if !make_command_result.success() {
        panic!("Failed to build Opus via `make`.");
    }

    let make_install_command_result = Command::new("make")
        .arg("install")
        .current_dir(&opus_path)
        .status()
        .expect("Failed to run `make install`.");

    if !make_install_command_result.success() {
        panic!("Failed to install Opus via `make install`.");
    }

    println!("cargo:rustc-link-lib={}=opus", is_static_text);
    println!(
        "cargo:rustc-link-search=native={}/lib",
        build_directory.display()
    );
}

#[cfg(all(windows, target_env = "msvc"))]
fn build_opus(_build_directory: &Path, is_static: bool, installed_lib_directory: &Option<String>) {
    link_prebuilt_opus(is_static, installed_lib_directory);
}

/// Links to prebuilt Windows library-files of Opus.
#[cfg(all(windows, target_env = "msvc"))]
fn link_prebuilt_opus(is_static: bool, installed_lib_directory: &Option<String>) {
    let is_static_text = rustc_linking_word(is_static);

    #[cfg(target_arch = "x86")]
    const ARCHITECTURE: &'static str = "x86";
    #[cfg(target_arch = "x86_64")]
    const ARCHITECTURE: &'static str = "x64";

    const OPUS_DLL: &'static str = "opus.dll";

    if let Some(prebuilt_directory) = installed_lib_directory {
        println!(
            "cargo:info=Prebuilt Opus will be linked: {}",
            prebuilt_directory
        );

        println!(
            "{}",
            format!("cargo:rustc-link-lib={}=opus", is_static_text)
        );
        println!("cargo:rustc-link-search=native={}", prebuilt_directory);

        return;
    }

    let mut building_path = Path::new("msvc").join(ARCHITECTURE);

    if !is_static {
        building_path = building_path.join("dy");
    }

    let library_path = building_path
        .canonicalize()
        .expect("Could not canonicalise.");

    println!("cargo:info=Try to build {} library.", is_static_text);
    println!("cargo:rustc-link-lib={}=opus", is_static_text);
    println!("cargo:rustc-link-search=native={}", library_path.display());

    if !is_static {
        building_path = building_path.join(OPUS_DLL);

        let dll_destination = find_cargo_target_dir();
        let dll_destination = dll_destination.join(OPUS_DLL);

        println!(
            "cargo:info=Found Cargo target directory: {:?}.",
            &dll_destination
        );

        std::fs::copy(&building_path, &dll_destination).expect(&format!(
            "Failed to copy `opus.dll` from `{}` to `{}`.",
            building_path.to_string_lossy(),
            dll_destination.to_string_lossy()
        ));
    }
}

#[cfg(all(windows, target_env = "msvc"))]
fn find_cargo_target_dir() -> PathBuf {
    let pkg_name =
        env::var("CARGO_PKG_NAME").expect("Environment variable `CARGO_PKG_NAME` is missing.");

    let mut out_dir =
        PathBuf::from(env::var("OUT_DIR").expect("Environment variable `OUT_DIR` is missing."));

    loop {
        let target_directory = out_dir.file_name().unwrap();

        if target_directory.to_string_lossy().contains(&pkg_name) {
            break;
        } else if !out_dir.pop() {
            panic!("Unexpected build path: {}", out_dir.to_string_lossy());
        }
    }

    out_dir.pop();
    out_dir.pop();

    out_dir
}

#[cfg(any(unix, target_env = "gnu"))]
fn find_via_pkg_config(is_static: bool) -> bool {
    pkg_config::Config::new()
        .statik(is_static)
        .probe("opus")
        .is_ok()
}

fn is_target_x32() -> bool {
    env::var("CARGO_CFG_TARGET_POINTER_WIDTH")
        .map(|var| var == "32")
        .unwrap_or(false)
}

fn is_target_os(os: &str) -> bool {
    env::var("CARGO_CFG_TARGET_OS")
        .map(|var| var == os)
        .unwrap_or(false)
}

fn is_target_family(family: &str) -> bool {
    env::var("CARGO_CFG_TARGET_FAMILY")
        .map(|var| var == family)
        .unwrap_or(false)
}

fn is_target_env(env: &str) -> bool {
    env::var("CARGO_CFG_TARGET_ENV")
        .map(|var| var == env)
        .unwrap_or(false)
}

fn is_cross_compiled() -> bool {
    let host_arch = env::var("CARGO_CFG_HOST_ARCH")
        .expect("Could not read host architecture environment variable.");

    println!("cargo:info=Host architecture: {:?}.", host_arch);

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH")
        .expect("Could not read target architecture environment variable.");

    println!("cargo:info=Target architecture: {:?}.", target_arch);

    host_arch != target_arch
}

/// Based on the OS or target environment we are building for,
/// this function will return an expected default library linking method.
///
/// If we build for Windows, MacOS, or Linux with musl, we will link statically.
/// However, if you build for Linux without musl, we will link dynamically.
///
/// **Info**:
/// This is a helper-function and may not be called if
/// if the `static`-feature is enabled, the environment variable
/// `LIBOPUS_STATIC` or `OPUS_STATIC` is set.
fn default_library_linking() -> bool {
    if is_target_family("windows") || is_target_os("macos") || is_target_env("musl") {
        true
    } else if is_target_family("unix") && is_target_env("gnu") {
        false
    } else {
        panic!("Unsupported target operating system.");
    }
}

fn find_installed_opus() -> Option<String> {
    if let Ok(lib_directory) = env::var("LIBOPUS_LIB_DIR") {
        Some(lib_directory)
    } else if let Ok(lib_directory) = env::var("OPUS_LIB_DIR") {
        Some(lib_directory)
    } else {
        None
    }
}

fn is_static_build() -> bool {
    if cfg!(feature = "static") && cfg!(feature = "dynamic") {
        default_library_linking()
    } else if cfg!(feature = "static")
        || env::var("LIBOPUS_STATIC").is_ok()
        || env::var("OPUS_STATIC").is_ok()
    {
        println!("cargo:info=Static feature or environment variable found.");

        true
    } else if cfg!(feature = "dynamic") {
        println!("cargo:info=Dynamic feature enabled.");

        false
    } else {
        println!("cargo:info=No feature or environment variable found, linking by default.");

        default_library_linking()
    }
}

fn main() {
    let installed_lib_directory = find_installed_opus();

    let is_static = is_static_build();

    #[cfg(any(unix, target_env = "gnu"))]
    {
        if env::var("LIBOPUS_NO_PKG").is_ok() || env::var("OPUS_NO_PKG").is_ok() {
            println!("cargo:info=Bypassed `pkg-config`.");
        } else if find_via_pkg_config(is_static) {
            println!("cargo:info=Found `Opus` via `pkg_config`.");

            return;
        } else {
            println!("cargo:info=`pkg_config` could not find `Opus`.");
        }
    }

    let build_variable =
        std::env::var("OUT_DIR").expect("Environment variable `OUT_DIR` is missing.");

    let build_path = Path::new(&build_variable);

    build_opus(&build_path, is_static, &installed_lib_directory);
}
