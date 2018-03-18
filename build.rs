extern crate regex;
extern crate zip;

use regex::Regex;
use std::env;
use std::io;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target = env::var("TARGET").unwrap();
    if target.contains("android") {
        android_main(&target);
    }
}

fn android_main(target: &str) {
    // Download zip file containing prebuilt libgstreamer_android.so
    let lib_file_name = if Regex::new("arm-([a-z])*-androideabi").unwrap().is_match(target) {
        "gst-build-armeabi"
    } else if Regex::new("armv7-([a-z])*-androideabi").unwrap().is_match(target) {
        "gst-build-armeabi-v7a"
    } else if Regex::new("x86_64-([a-z])*-android").unwrap().is_match(target) {
        "gst-build-x86_g4"
    } else {
        panic!("Invalid target architecture {}", target);
    };

    let url = format!("https://github.com/ferjm/libgstreamer_android_gen/blob/gh-pages/out/{}.zip?raw=true",
                      lib_file_name);
    let status = Command::new("wget").args(&[&url, "-O", "lib.zip"]).status().unwrap();
    if !status.success() {
        panic!("Could not download required libgstreamer_android.so {}", status);
    }

    // Unpack downloaded lib zip
    let fname = std::path::Path::new("lib.zip");
    let file = fs::File::open(&fname).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let mut outpath = PathBuf::from("target");
        outpath.push(sanitize_filename(file.name()));
        if (&*file.name()).ends_with('/') {
            println!("File {} extracted to \"{}\"", i, outpath.as_path().display());
            fs::create_dir_all(&outpath).unwrap();
        } else {
            println!("File {} extracted to \"{}\" ({} bytes)", i, outpath.as_path().display(), file.size());
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p).unwrap();
                }
            }
            let mut outfile = fs::File::create(&outpath).unwrap();
            io::copy(&mut file, &mut outfile).unwrap();
        }

        // Get and Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode)).unwrap();
            }
        }
    }

    // Remove downloaded zip
    fs::remove_file("lib.zip").unwrap();

    // Change pkgconfig info to make all GStreamer dependencies point to
    // libgstreamer_android.so
    let mut lib_dir = env::current_dir().unwrap();
    lib_dir.push("target");
    lib_dir.push(lib_file_name);
    let mut pkg_config_dir = lib_dir.clone();
    let lib_dir_str = lib_dir.to_str().unwrap();
    let expr = format!("'s?libdir=.*?libdir={}?g'", &lib_dir_str);
    pkg_config_dir.push("pkgconfig");
    pkg_config_dir.push("*");
    let pkg_config_dir_str = pkg_config_dir.to_str().unwrap();
    let status = Command::new("perl").arg("-i").arg("-pe").arg(&expr).arg(&pkg_config_dir_str).status().unwrap();
    if !status.success() {
        panic!("Could not modify pkgconfig data {}", status);
    }
}

fn sanitize_filename(filename: &str) -> std::path::PathBuf {
    let no_null_filename = match filename.find('\0') {
        Some(index) => &filename[0..index],
        None => filename,
    };

    std::path::Path::new(no_null_filename)
        .components()
        .filter(|component| match *component {
            std::path::Component::Normal(..) => true,
            _ => false,
        })
    .fold(std::path::PathBuf::new(), |mut path, ref cur| {
        path.push(cur.as_os_str());
        path
    })
}
