//! Download compliance data during build process

fn main() {
    #[cfg(feature = "compliance-tests")]
    download_compliance_data();
}

#[cfg(feature = "compliance-tests")]
fn download_compliance_data() {
    use std::path::PathBuf;

    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("compliance-data-cache");
    let marker = data_dir.join(".downloaded_v1");

    if marker.exists() {
        println!("cargo::warning=Using cached compliance data");
        return;
    }

    println!("cargo::warning=Downloading compliance test data (~130MB, one-time download)...");

    let url = "https://github.com/uclouvain/openjpeg-data/archive/39524bd3a601d90ed8e0177559400d23945f96a9.zip";
    println!(
        "cargo::warning=Downloading compliance test data from: {}",
        url
    );
    std::fs::create_dir_all(&data_dir).unwrap();

    let zip_path = data_dir.join("temp.zip");

    let status = std::process::Command::new("curl")
        .args(&["-L", "-o", zip_path.to_str().unwrap(), url])
        .status()
        .expect("Failed to download compliance data (is curl installed?)");

    assert!(status.success(), "Download failed");

    let status = std::process::Command::new("unzip")
        .args(&[
            "-q",
            zip_path.to_str().unwrap(),
            "-d",
            data_dir.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to extract (is unzip installed?)");

    assert!(status.success(), "Extraction failed");

    std::fs::remove_file(zip_path).ok();
    std::fs::write(marker, env!("CARGO_PKG_VERSION")).unwrap();

    println!(
        "cargo::warning=Compliance data ready at: {}",
        data_dir.display()
    );
}
