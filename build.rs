extern crate bindgen;
extern crate bzip2;
extern crate cc;
extern crate glob;
extern crate libflate;
extern crate pkg_config;
extern crate reqwest;
extern crate tar;
extern crate url;

use bzip2::read::BzDecoder;
use glob::glob;
use libflate::gzip;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;
use url::Url;

struct Library {
    dynamic: Option<bool>,
    libs: Vec<String>,
    link_paths: Vec<PathBuf>,
    frameworks: Vec<String>,
    framework_paths: Vec<PathBuf>,
    include_paths: Vec<PathBuf>,
    defines: HashMap<String, Option<String>>,
}

fn pkg_config_find_library(version: String, dynamic: Option<bool>) -> Option<Library> {
    let mut config = pkg_config::Config::new();
    config.atleast_version(&version);
    if dynamic == Some(false) {
        config.statik(true);
    }
    match config.probe("openh264") {
        Ok(pkg_config_library) => Some(Library {
            dynamic: dynamic,
            libs: pkg_config_library.libs,
            link_paths: pkg_config_library.link_paths,
            frameworks: pkg_config_library.frameworks,
            framework_paths: pkg_config_library.framework_paths,
            include_paths: pkg_config_library.include_paths,
            defines: pkg_config_library.defines,
        }),
        _ => None,
    }
}

fn extract_source(out_dir_path: &Path, version: &str) -> String {
    let archive_dir_path = out_dir_path.join("archive");
    if !archive_dir_path.exists() {
        std::fs::create_dir(&archive_dir_path)
            .expect(&format!("Failed to create {:?}", archive_dir_path));
    }

    let archive_file_path = archive_dir_path.join(format!("openh264-{}.tar.gz", version));
    if !archive_file_path.exists() {
        let url = format!(
            "https://github.com/cisco/openh264/archive/v{}.tar.gz",
            version
        );
        let mut response = reqwest::get(&url).expect(&format!("Failed to download {}", url));
        assert!(
            response.status().is_success(),
            format!("Request to {} doesn't succeed: {}", url, response.status())
        );
        let mut file_buf = Vec::new();
        response
            .copy_to(&mut file_buf)
            .expect(&format!("Failed to download {}", url));
        let mut file = File::create(&archive_file_path)
            .expect(&format!("Failed to create {:?}", archive_file_path));
        file.write_all(&file_buf).expect(&format!(
            "Failed to save {} to {:?}",
            url, archive_file_path
        ));
    }

    let mut archive_file =
        File::open(&archive_file_path).expect(&format!("Failed to open {:?}", archive_file_path));

    let mut gzip_decoder = gzip::Decoder::new(&mut archive_file).expect(&format!(
        "Failed to create gzip decoder for {:?}",
        archive_file_path
    ));
    let mut tar_vec = Vec::new();
    std::io::copy(&mut gzip_decoder, &mut tar_vec).expect(&format!(
        "Failed to extract gzip archive {:?}",
        archive_file_path
    ));

    let mut tar_archive = Archive::new(&tar_vec[..]);
    let tar_extract_dir_path = out_dir_path.join("src");
    if tar_extract_dir_path.exists() {
        std::fs::remove_dir_all(tar_extract_dir_path.clone()).expect(&format!(
            "Failed to remove old archive extraction dir: {:?}",
            tar_extract_dir_path.clone()
        ));
    }
    for mut entry in tar_archive
        .entries()
        .expect(&format!(
            "Failed to read tar archive entries in {:?}",
            archive_file_path
        ))
        .map(|entry| {
            entry.expect(&format!(
                "Failed to extract tar archive entry in {:?}",
                archive_file_path
            ))
        }) {
        entry
            .unpack_in(tar_extract_dir_path.clone())
            .expect(&format!(
                "Failed to unpack file in {:?} for {:?}",
                archive_file_path,
                entry.path()
            ));
    }

    let openh264_src_dir_path = std::fs::read_dir(&tar_extract_dir_path)
        .expect(&format!("Failed to read dir {:?}", tar_extract_dir_path))
        .map(|entry| {
            entry.expect(&format!(
                "Failed to read dir entry in {:?}",
                tar_extract_dir_path
            ))
        })
        .find(|entry| {
            entry
                .file_type()
                .expect(&format!(
                    "Failed to read file type for {:?} in {:?}",
                    entry, tar_extract_dir_path
                ))
                .is_dir()
        }).expect(
            &format!("Failed to find openh264 extracted src path in {:?}, perhaps downloaded archive {:?} was broken.",
            tar_extract_dir_path, archive_file_path)
        ).path();
    openh264_src_dir_path
        .to_str()
        .expect(&format!(
            "Failed to extract rust string from {:?}",
            openh264_src_dir_path
        ))
        .to_string()
}

fn find_make() -> Option<String> {
    ["make", "gmake"]
        .iter()
        .find(|make| match Command::new(make).arg("--version").status() {
            Ok(status) => status.success(),
            _ => false,
        })
        .map(|s| s.to_string())
}

fn make_unix_path(path_str: &str) -> String {
    if cfg!(windows) {
        path_str.replace("\\", "/")
    } else {
        path_str.to_owned()
    }
}

fn build_library(out_dir_path: &Path, version: &str, dynamic: Option<bool>) -> Library {
    let mut library = Library {
        dynamic: dynamic,
        libs: vec!["openh264".to_owned()],
        link_paths: Vec::new(),
        frameworks: Vec::new(),
        framework_paths: Vec::new(),
        include_paths: Vec::new(),
        defines: HashMap::new(),
    };

    let prefix_dir_path = out_dir_path.join("prefix");
    let prefix_include_dir_path = prefix_dir_path.join("include");
    let prefix_lib_dir_path = prefix_dir_path.join("lib");

    library.include_paths.push(prefix_include_dir_path.clone());
    library.link_paths.push(prefix_lib_dir_path.clone());

    let done_file_path = prefix_dir_path.join("build_done");
    if done_file_path.exists() {
        return library;
    }

    let openh264_src_dir_path_str = extract_source(out_dir_path, version);

    if prefix_dir_path.exists() {
        std::fs::remove_dir_all(prefix_dir_path.clone()).expect(&format!(
            "Failed to remove old installation prefix dir: {:?}",
            prefix_dir_path.clone()
        ));
    }
    std::fs::create_dir(&prefix_dir_path)
        .expect(&format!("Failed to create {:?}", prefix_dir_path));

    let prefix_dir_str = prefix_dir_path.to_str().expect(&format!(
        "Failed to extract rust string from {:?}",
        prefix_dir_path
    ));

    let make = find_make().expect("Unable find `make' or `gmake' command");
    let make_task = &(if dynamic == Some(false) {
        "install-static"
    } else {
        "install-shared"
    }).to_owned();
    let make_status = Command::new(&make)
        .current_dir(&openh264_src_dir_path_str)
        .args(&vec![
            &format!("PREFIX={}", make_unix_path(prefix_dir_str)),
            make_task,
        ])
        .status()
        .expect(&format!(
            "Failed to execute `make {}' for openh264",
            make_task
        ));
    if !make_status.success() {
        panic!("Failed to execute `make {}', status code: {}. Please see around the build output dir {:?}", make_task, make_status, out_dir_path);
    }

    let _ = File::create(&done_file_path).expect(&format!("Failed to create {:?}", done_file_path));

    library
}

fn find_prebuilt_library(
    full_version: &str,
    major_version: &str,
) -> (String, String, String, String) {
    if cfg!(target_os = "android") {
        (
            format!("libopenh264-{}-android19.so.bz2", full_version),
            format!("libopenh264.so"),
            format!("libopenh264.so"),
            format!("libopenh264.so"),
        )
    } else if cfg!(all(target_os = "linux", target_arch = "x86")) {
        (
            format!(
                "libopenh264-{}-linux32.{}.so.bz2",
                full_version, major_version
            ),
            format!("libopenh264.so.{}", full_version),
            format!("libopenh264.so.{}", major_version),
            format!("libopenh264.so"),
        )
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        (
            format!(
                "libopenh264-{}-linux64.{}.so.bz2",
                full_version, major_version
            ),
            format!("libopenh264.so.{}", full_version),
            format!("libopenh264.so.{}", major_version),
            format!("libopenh264.so"),
        )
    } else if cfg!(all(target_os = "macos", target_arch = "x86")) {
        (
            format!(
                "libopenh264-{}-osx32.{}.dylib.bz2",
                full_version, major_version
            ),
            format!("libopenh264.{}.dylib", full_version),
            format!("libopenh264.{}.dylib", major_version),
            format!("libopenh264.dylib"),
        )
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        (
            format!(
                "libopenh264-{}-osx64.{}.dylib.bz2",
                full_version, major_version
            ),
            format!("libopenh264.{}.dylib", full_version),
            format!("libopenh264.{}.dylib", major_version),
            format!("libopenh264.dylib"),
        )
    } else if cfg!(all(target_os = "windows", target_arch = "x86")) {
        (
            format!("openh264-{}-win32.dll.bz2", full_version),
            format!("openh264.dll"),
            format!("openh264.dll"),
            format!("openh264.dll"),
        )
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        (
            format!("openh264-{}-win64.dll.bz2", full_version),
            format!("openh264.dll"),
            format!("openh264.dll"),
            format!("openh264.dll"),
        )
    } else {
        panic!("Prebuilt binary is not found in github releases. Please install libopenh264 manually or enable `build' cargo feature to build from source.");
    }
}

fn download_library(out_dir_path: &Path, full_version: &str, major_version: &str) -> Library {
    let mut library = Library {
        dynamic: Some(true),
        libs: vec!["openh264".to_owned()],
        link_paths: Vec::new(),
        frameworks: Vec::new(),
        framework_paths: Vec::new(),
        include_paths: Vec::new(),
        defines: HashMap::new(),
    };

    let prefix_dir_path = out_dir_path.join("prefix");
    let prefix_include_dir_path = prefix_dir_path.join("include");
    let prefix_lib_dir_path = prefix_dir_path.join("lib");

    library.include_paths.push(prefix_include_dir_path.clone());
    library.link_paths.push(prefix_lib_dir_path.clone());

    let done_file_path = prefix_dir_path.join("download_done");
    if done_file_path.exists() {
        return library;
    }

    let (archive_file_name, full_version_so_name, major_version_so_name, short_so_name) =
        find_prebuilt_library(full_version, major_version);

    let url_str = format!(
        "https://github.com/cisco/openh264/releases/download/v{}/{}",
        full_version, archive_file_name
    );

    let openh264_src_dir_path_str = extract_source(out_dir_path, full_version);

    let archive_dir_path = out_dir_path.join("archive");
    if !archive_dir_path.exists() {
        std::fs::create_dir(&archive_dir_path)
            .expect(&format!("Failed to create {:?}", archive_dir_path));
    }

    let url = Url::parse(&url_str).expect(&format!("Failed to parse url string: {}", url_str));
    let archive_file_path = archive_dir_path.join(archive_file_name);
    if !archive_file_path.exists() {
        let mut response =
            reqwest::get(url.as_str()).expect(&format!("Failed to download {}", url));
        assert!(
            response.status().is_success(),
            format!("request for {} doesn't succeed: {}", url, response.status())
        );
        let mut file_buf = Vec::new();
        response
            .copy_to(&mut file_buf)
            .expect(&format!("Failed to download {}", url));
        let mut file = File::create(&archive_file_path)
            .expect(&format!("Failed to create {:?}", archive_file_path));
        file.write_all(&file_buf).expect(&format!(
            "Failed to save {} to {:?}",
            url, archive_file_path
        ));
    }

    let mut archive_file =
        File::open(&archive_file_path).expect(&format!("Failed to open {:?}", archive_file_path));

    let mut bzip2_decoder = BzDecoder::new(&mut archive_file);
    let mut so_vec = Vec::new();
    std::io::copy(&mut bzip2_decoder, &mut so_vec).expect(&format!(
        "Failed to extract bzip2 archive {:?}",
        archive_file_path
    ));

    if prefix_dir_path.exists() {
        std::fs::remove_dir_all(prefix_dir_path.clone()).expect(&format!(
            "Failed to remove old installation prefix dir: {:?}",
            prefix_dir_path.clone()
        ));
    }
    std::fs::create_dir(&prefix_dir_path)
        .expect(&format!("Failed to create {:?}", prefix_dir_path));
    std::fs::create_dir(&prefix_lib_dir_path)
        .expect(&format!("Failed to create {:?}", prefix_lib_dir_path));
    let so_path = prefix_lib_dir_path.join(&full_version_so_name);
    let mut so_file = File::create(&so_path).expect(&format!(
        "Failed to create prebuilt binary to {:?}",
        so_path
    ));
    so_file
        .write_all(&so_vec)
        .expect(&format!("Failed to save prebuilt binary to {:?}", so_path));
    #[cfg(unix)]
    {
        let metadata = so_file
            .metadata()
            .expect(&format!("Failed to read permissions for {:?}", so_path));
        let mut permissions = metadata.permissions();
        let mut mode = permissions.mode();
        mode |= 0o100;
        if mode & 0o040 != 0 {
            mode |= 0o010;
        }
        if mode & 0o004 != 0 {
            mode |= 0o001;
        }
        permissions.set_mode(mode);
        so_file.set_permissions(permissions).expect(&format!(
            "Failed to set permission {} to {:?}",
            mode, so_path,
        ));
    }

    for &(target_so_name, link_so_name) in [
        (&full_version_so_name, &major_version_so_name),
        (&major_version_so_name, &short_so_name),
    ].iter()
    {
        if target_so_name == link_so_name {
            continue;
        }
        let target_path = prefix_lib_dir_path.join(target_so_name);
        let link_path = prefix_lib_dir_path.join(link_so_name);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target_path, &link_path).expect(&format!(
                "Failed to create symlink target: {:?}, linkname: {:?}",
                so_path, link_path
            ));
        }
        #[cfg(not(unix))]
        {
            panic!(
                "Internal logic error: Using symlink on non-unix target: {:?}, linkname: {:?}",
                target_path, link_path
            );
        }
    }

    let prefix_dir_str = prefix_dir_path.to_str().expect(&format!(
        "Failed to extract rust string from {:?}",
        prefix_dir_path,
    ));

    match find_make() {
        Some(make) => {
            let make_status = Command::new(make)
                .current_dir(&openh264_src_dir_path_str)
                .args(&vec![
                    format!("PREFIX={}", make_unix_path(prefix_dir_str)),
                    "install-headers".to_owned(),
                ])
                .status()
                .expect("Failed to execute `make install-headers' for openh264");
            if !make_status.success() {
                panic!("Failed to execute `make install-headers', status code: {}. Please see around the build output dir {:?}", make_status, out_dir_path);
            }
        }
        None => {
            if !prefix_include_dir_path.exists() {
                std::fs::create_dir(&prefix_include_dir_path)
                    .expect(&format!("Failed to create {:?}", prefix_include_dir_path));
            }

            let prefix_include_wels_dir_path = prefix_include_dir_path.join("wels");
            if !prefix_include_wels_dir_path.exists() {
                std::fs::create_dir(&prefix_include_wels_dir_path).expect(&format!(
                    "Failed to create {:?}",
                    prefix_include_wels_dir_path
                ));
            }

            // https://github.com/cisco/openh264/blob/v1.7.0/Makefile#L290-L292
            let src_headers_glob = format!("{}/codec/api/svc/codec*.h", openh264_src_dir_path_str);
            for entry_result in glob(&src_headers_glob).expect(&format!(
                "Failed to create installation header list with glob: {}",
                src_headers_glob
            )) {
                let entry = entry_result.expect(&format!(
                    "Failed to read installation header entry in {}",
                    src_headers_glob
                ));
                let from_path: &Path = entry.as_ref();
                let to_path = prefix_include_wels_dir_path.clone().join(
                    from_path.file_name().expect(&format!(
                        "Failed to extract rust string from path: {:?}",
                        from_path
                    )),
                );
                std::fs::copy(&from_path, &to_path)
                    .expect(&format!("Failed to copy {:?} to {:?}", from_path, to_path));
            }
        }
    }

    if cfg!(target_env = "msvc") {
        let build = cc::Build::new();
        let compiler = build.get_compiler();
        let prefix_lib_dir_path_str = prefix_lib_dir_path.to_str().expect(&format!(
            "Failed to extract rust string from path: {:?}",
            prefix_lib_dir_path
        ));
        let compiler_path = compiler.path();
        let compiler_dir_path = compiler_path.parent().expect(&format!(
            "Couldn't find compiler base directory. compiler path = {:?}",
            compiler_path
        ));
        let lib_command = compiler_dir_path.join("lib");
        let lib_status = Command::new(&lib_command)
            .args(&vec![
                &format!("/DEF:{}\\openh264.def", openh264_src_dir_path_str),
                &format!("/OUT:{}\\openh264.lib", prefix_lib_dir_path_str),
            ])
            .status()
            .expect(&format!(
                "Failed to execute `{:?}' to generate openh264.lib",
                lib_command
            ));
        if !lib_status.success() {
            panic!("Failed to execute `{:?}', status code: {}. Please see around the build output dir {:?}", lib_command, lib_status, 
            out_dir_path);
        }
    }

    let _ = File::create(&done_file_path).expect(&format!("Failed to create {:?}", done_file_path));

    library
}

fn find_or_build_library(out_dir_path: &Path) -> Library {
    let full_version = "1.7.0";
    let major_version = "4";
    let dynamic = if cfg!(feature = "static") {
        Some(false)
    } else {
        None
    };

    if cfg!(feature = "build") {
        if cfg!(windows) {
            panic!("feature `build' is currently unimplemented for Windows");
        }
        return build_library(out_dir_path, full_version, dynamic);
    }

    match (env::var("OPENH264_INCLUDE_PATH"), env::var("OPENH264_LIBRARY_PATH")) {
        (Ok(include_path), Ok(library_path)) => {
            return Library {
                dynamic: dynamic,
                libs: vec!["openh264".to_owned()],
                link_paths: vec![PathBuf::from(library_path)],
                frameworks: Vec::new(),
                framework_paths: Vec::new(),
                include_paths: vec![PathBuf::from(include_path)],
                defines: HashMap::new(),
            }
        },
        (Ok(_), _) => panic!("Environment variable `OPENH264_INCLUDE_PATH' exists but `OPENH264_LIBRARY_PATH' doesn't exist. Both variables are required."),
        (_, Ok(_)) => panic!("Environment variable `OPENH264_LIBRARY_PATH' exists but `OPENH264_INCLUDE_PATH' doesn't exist. Both variables are required."),
        _ => {},
    }

    match pkg_config_find_library(full_version.to_owned(), dynamic) {
        Some(library) => library,
        None => {
            if dynamic == Some(false) {
                panic!("Unable to find openh264 library")
            }

            let library = download_library(out_dir_path, full_version, major_version);
            print_linker_flags(&library);
            library
        }
    }
}

fn generate_bindings(library: &Library, out_dir_path: &Path) {
    let mut bindgen_builder = bindgen::Builder::default()
        .header("wrapper.h")
        .derive_default(true)
        .prepend_enum_name(false);

    for include_path in &library.include_paths {
        bindgen_builder = bindgen_builder.clang_arg(format!(
            "-I{}",
            include_path.to_str().expect(&format!(
                "Failed to extract rust string from include_path={:?}",
                include_path
            ))
        ));
    }

    for (define_key, define_value_option) in &library.defines {
        bindgen_builder = if let Some(define_value) = define_value_option {
            bindgen_builder.clang_args(&["-D", &format!("{}={}", define_key, define_value)])
        } else {
            bindgen_builder.clang_args(&["-D", define_key])
        }
    }

    let bindings_file = out_dir_path.join("bindings.rs");
    bindgen_builder
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(&bindings_file)
        .expect(&format!("Couldn't write bindings to {:?}", &bindings_file));
}

fn print_linker_flags(library: &Library) {
    for framework_path in &library.framework_paths {
        println!(
            "cargo:rustc-link-search=framework={}",
            framework_path.to_str().expect(&format!(
                "Failed to extract rust string from framework_path={:?}",
                framework_path
            ))
        );
    }

    for framework in &library.frameworks {
        println!("cargo:rustc-link-lib=framework={}", framework);
    }

    for link_path in &library.link_paths {
        println!(
            "cargo:rustc-link-search=native={}",
            link_path.to_str().expect(&format!(
                "Failed to extract rust string from link_path={:?}",
                link_path
            ))
        );
    }

    for lib in &library.libs {
        if library.dynamic == Some(false) {
            println!("cargo:rustc-link-lib=static={}", lib);
        } else {
            println!("cargo:rustc-link-lib=dylib={}", lib);
        }
    }
}

fn main() {
    let out_dir = env::var("OUT_DIR").expect("Failed to find environment variable OUT_DIR");
    let out_dir_path = Path::new(&out_dir);
    let library = find_or_build_library(&out_dir_path);
    generate_bindings(&library, &out_dir_path);
}
