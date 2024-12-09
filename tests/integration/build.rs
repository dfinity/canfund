use lazy_static::lazy_static;
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{
    env,
    fs::{self, File},
    io::{self, ErrorKind, Write},
    path::Path,
    process::{Command, Stdio},
};

const POCKET_IC_BIN_NAME: &str = "pocket-ic";

lazy_static! {
    static ref CURRENT_DIR: PathBuf = {
        let current_dir =
            env::var("CARGO_MANIFEST_DIR").expect("Failed to get current directory path from env");

        PathBuf::from(current_dir)
    };

    static ref WASM_OUT_DIR: PathBuf = CURRENT_DIR.join("wasms");
    static ref OUT_DIR: PathBuf = CURRENT_DIR.to_path_buf();

    static ref WORKSPACE_ROOT: PathBuf = {
        let manifest_path = CURRENT_DIR.join("Cargo.toml");

        // Retrieve metadata for the specified package
        let output = Command::new("cargo")
            .args([
                "metadata",
                "--no-deps",
                "--format-version=1",
                "--manifest-path",
                manifest_path.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to execute cargo metadata");

        // Parse the JSON output
        let metadata: Value = serde_json::from_slice(&output.stdout)
            .expect("Failed to parse JSON from cargo metadata output");

        let workspace_root = metadata["workspace_root"]
            .as_str()
            .expect("Failed to get workspace root")
            .to_string();

        PathBuf::from(workspace_root)
    };
    static ref OS_TYPE: String = {
        let os = std::env::var("CARGO_CFG_TARGET_OS").expect("Failed to get OS type from env");

        if os == "macos" {
            "darwin".to_string()
        } else if os == "linux" {
            "linux".to_string()
        } else {
            eprintln!("Unsupported OS: {}", os);
            std::process::exit(1);
        }
    };

    static ref DEPENDENCIES: Vec<PathBuf> =
        vec![WORKSPACE_ROOT.join("examples"), WORKSPACE_ROOT.join("canfund-rs")];

    static ref EXAMPLES_WASMS : Vec<String> = {
        let mut packages = Vec::new();
        let examples_path = WORKSPACE_ROOT.join("examples");

        for entry in fs::read_dir(examples_path).expect("Failed to read serializers directory") {
            let entry = entry.expect("Failed to read entry");
            let path = entry.path();

            if path.is_dir() {
                let cargo_toml = path.join("Cargo.toml");

                if cargo_toml.exists() {
                    let package_name = toml::from_str::<toml::Value>(
                        &fs::read_to_string(cargo_toml).expect("Failed to read Cargo.toml"),
                    )
                    .expect("Failed to parse Cargo.toml")
                    .get("package")
                    .expect("Failed to get package")
                    .get("name")
                    .expect("Failed to get name")
                    .as_str()
                    .expect("Failed to get name")
                    .to_string();

                    packages.push(package_name);
                }
            }
        }

        packages
    };
}

pub fn build_wasm(package: &str) {
    let status = Command::new("cargo")
        .args([
            "build",
            "--locked",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--package",
            package,
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute cargo build command");

    // Check if the command was executed successfully
    if !status.success() {
        eprintln!("Failed to build package {}", package);
        std::process::exit(1);
    }

    let wasm_file = WORKSPACE_ROOT.join(format!(
        "target/wasm32-unknown-unknown/release/{}.wasm",
        package.replace('-', "_")
    ));
    let out_file = WASM_OUT_DIR.join(format!("{}.wasm", package));

    fs::rename(wasm_file, out_file.clone()).expect("Failed to move wasm file");
    fs::set_permissions(out_file.clone(), fs::Permissions::from_mode(0o755))
        .expect("Failed to set permissions");

    // shrink wasm file
    let status = Command::new(OUT_DIR.join("ic-wasm"))
        .args([
            "-o",
            out_file.to_str().unwrap(),
            out_file.to_str().unwrap(),
            "shrink",
        ])
        .status()
        .expect("Failed to execute ic-wasm command");

    if !status.success() {
        eprintln!("Failed to shrink wasm file");
        std::process::exit(1);
    }

    // compress wasm file with gzip
    let status = Command::new("gzip")
        .args(["-f", out_file.to_str().unwrap()])
        .status()
        .expect("Failed to execute gzip command");

    if !status.success() {
        eprintln!("Failed to compress wasm file");
        std::process::exit(1);
    }
}

fn download(url: &str, destination: &Path) -> io::Result<()> {
    let response = reqwest::blocking::get(url).expect("Request failed");
    if response.status().is_success() {
        let mut dest = File::create(destination)?;
        let content = response.bytes().expect("Failed to read response bytes");
        dest.write_all(&content)?;
        Ok(())
    } else {
        Err(io::Error::new(
            ErrorKind::Other,
            format!("Failed to download file: {}", url),
        ))
    }
}

pub fn download_ic_wasm() {
    if OUT_DIR.join("ic-wasm").exists() {
        println!("IC Wasm already available, skipping download");
        return;
    }

    let ic_wasm_url =
        match OS_TYPE.as_str() {
            "darwin" => "https://github.com/dfinity/ic-wasm/releases/download/0.6.0/ic-wasm-macos"
                .to_string(),
            "linux" => "https://github.com/dfinity/ic-wasm/releases/download/0.6.0/ic-wasm-linux64"
                .to_string(),
            _ => {
                eprintln!("Unsupported OS: {}", OS_TYPE.as_str());
                std::process::exit(1);
            }
        };

    download(&ic_wasm_url, &OUT_DIR.join("ic-wasm")).expect("Failed to download ic-wasm");

    fs::set_permissions(OUT_DIR.join("ic-wasm"), fs::Permissions::from_mode(0o755))
        .expect("Failed to set permissions");
}

pub fn download_pocket_ic() {
    if OUT_DIR.join(POCKET_IC_BIN_NAME).exists() {
        println!("PocketIC already available, skipping download");
        return;
    }

    let pocket_ic_url = format!(
        "https://github.com/dfinity/pocketic/releases/download/7.0.0/pocket-ic-x86_64-{}.gz",
        OS_TYPE.as_str()
    );
    let output_path = OUT_DIR.join(format!("pocket-ic-x86_64-{}.gz", OS_TYPE.as_str()));

    download(&pocket_ic_url, &output_path).expect("Failed to download pocket-ic");

    let status = Command::new("gzip")
        .args(["-df", output_path.to_str().unwrap()])
        .status()
        .expect("Failed to execute gzip command");

    if !status.success() {
        eprintln!("Failed to unzip pocket-ic");
        std::process::exit(1);
    }

    let uncompressed_path = output_path.with_extension("");

    fs::rename(uncompressed_path, OUT_DIR.join(POCKET_IC_BIN_NAME)).expect("Failed to rename file");
    fs::set_permissions(
        OUT_DIR.join(POCKET_IC_BIN_NAME),
        fs::Permissions::from_mode(0o755),
    )
    .expect("Failed to set permissions");

    println!("PocketIC download completed");
}

pub fn download_icp_ledger_wasm() {
    if WASM_OUT_DIR.join("icp_ledger.wasm.gz").exists() {
        println!("ICP Ledger Wasm already available, skipping download");
        return;
    }

    let icp_ledger_wasm_url = "https://download.dfinity.systems/ic/3d6a76efba59d6f03026d6b7c1c9a1dfce96ee93/canisters/ledger-canister.wasm.gz";

    download(
        icp_ledger_wasm_url,
        &WASM_OUT_DIR.join("icp_ledger.wasm.gz"),
    )
    .expect("Failed to download icp_ledger.wasm.gz");
}

pub fn download_cmc_wasm() {
    if WASM_OUT_DIR.join("cmc.wasm.gz").exists() {
        println!("ICP CMC Wasm already available, skipping download");
        return;
    }

    let cmc_wasm_url = "https://download.dfinity.systems/ic/3d6a76efba59d6f03026d6b7c1c9a1dfce96ee93/canisters/cycles-minting-canister.wasm.gz";

    download(cmc_wasm_url, &WASM_OUT_DIR.join("cmc.wasm.gz"))
        .expect("Failed to download cmc.wasm.gz");
}

pub fn main() {
    for dep in DEPENDENCIES.iter() {
        println!("cargo:rerun-if-changed={}", dep.display());
    }

    download_pocket_ic();
    download_ic_wasm();
    download_icp_ledger_wasm();
    download_cmc_wasm();

    // Build all wasm packages that are part of the benchmark suite
    for package in EXAMPLES_WASMS.iter() {
        build_wasm(package);
    }
}
