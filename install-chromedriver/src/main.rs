use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use serde::Deserialize;

fn get_chrome_version() -> Result<String, String> {
    let output = Command::new("google-chrome")
        .arg("--version")
        .output()
        .map_err(|e| format!("Failed to run google-chrome: {e}"))?;

    if !output.status.success() {
        return Err("google-chrome --version returned non-zero exit code".into());
    }

    // Output looks like "Google Chrome 143.0.7499.169 \n"
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout
        .split_whitespace()
        .last()
        .ok_or("Could not parse Chrome version from output")?
        .to_string();

    Ok(version)
}

fn get_chromedriver_version() -> Option<String> {
    let output = Command::new("chromedriver").arg("--version").output().ok()?;

    if !output.status.success() {
        return None;
    }

    // Output looks like "ChromeDriver 143.0.7499.169 (...)"
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split_whitespace()
        .nth(1)
        .map(|s| s.to_string())
}

#[derive(Deserialize)]
struct KnownGoodVersions {
    versions: Vec<CftVersion>,
}

#[derive(Deserialize)]
struct CftVersion {
    version: String,
    downloads: Downloads,
}

#[derive(Deserialize)]
struct Downloads {
    #[serde(default)]
    chromedriver: Vec<PlatformDownload>,
}

#[derive(Deserialize)]
struct PlatformDownload {
    platform: String,
    url: String,
}

/// Parse a version string into (major, minor, build, patch) tuple for comparison.
fn parse_version(v: &str) -> Option<(u64, u64, u64, u64)> {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
        parts[3].parse().ok()?,
    ))
}

/// Query the Chrome for Testing API and return the download URL for the best matching
/// chromedriver version (same major, closest patch, linux64).
fn find_chromedriver_url_from_api(chrome_version: &str) -> Result<String, String> {
    let api_url = "https://googlechromelabs.github.io/chrome-for-testing/known-good-versions-with-downloads.json";
    println!("Querying Chrome for Testing API for available chromedriver versions...");

    let response = reqwest::blocking::get(api_url)
        .map_err(|e| format!("Failed to query Chrome for Testing API: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Chrome for Testing API returned HTTP {}", response.status()));
    }

    let data: KnownGoodVersions = response
        .json()
        .map_err(|e| format!("Failed to parse Chrome for Testing API response: {e}"))?;

    let target = parse_version(chrome_version)
        .ok_or_else(|| format!("Could not parse Chrome version: {chrome_version}"))?;

    // Find all versions with the same major that have a linux64 chromedriver download,
    // then pick the one whose (major, minor, build, patch) is closest but <= target.
    let mut best: Option<(u64, u64, u64, u64, String)> = None;

    for entry in &data.versions {
        let Some(v) = parse_version(&entry.version) else { continue };
        if v.0 != target.0 {
            continue;
        }
        let Some(dl) = entry.downloads.chromedriver.iter().find(|d| d.platform == "linux64") else {
            continue;
        };
        if v <= target {
            match &best {
                None => best = Some((v.0, v.1, v.2, v.3, dl.url.clone())),
                Some(b) if v > (b.0, b.1, b.2, b.3) => {
                    best = Some((v.0, v.1, v.2, v.3, dl.url.clone()));
                }
                _ => {}
            }
        }
    }

    match best {
        Some((maj, min, build, patch, url)) => {
            println!("Using chromedriver {maj}.{min}.{build}.{patch} (closest available for Chrome {chrome_version})");
            Ok(url)
        }
        None => Err(format!(
            "No chromedriver found in Chrome for Testing API for Chrome major version {}",
            target.0
        )),
    }
}

fn main() {
    // Step 1: Get Chrome version
    println!("Checking installed Chrome version...");
    let chrome_version = match get_chrome_version() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };
    println!("Chrome version: {chrome_version}");

    // Step 2: Check chromedriver version
    println!("Checking installed chromedriver version...");
    if let Some(driver_version) = get_chromedriver_version() {
        println!("chromedriver version: {driver_version}");
        if driver_version == chrome_version {
            println!("Already up to date.");
            return;
        }
        println!("Version mismatch — updating chromedriver.");
    } else {
        println!("chromedriver not found or could not determine version.");
    }

    // Step 3: Download chromedriver zip
    let direct_url = format!(
        "https://storage.googleapis.com/chrome-for-testing-public/{}/linux64/chromedriver-linux64.zip",
        chrome_version
    );
    println!("Downloading {direct_url}");

    let tmp_dir = tempfile::tempdir().unwrap_or_else(|e| {
        eprintln!("Error creating temp directory: {e}");
        std::process::exit(1);
    });

    let zip_path = tmp_dir.path().join("chromedriver-linux64.zip");

    let response = reqwest::blocking::get(&direct_url).unwrap_or_else(|e| {
        eprintln!("Error downloading chromedriver: {e}");
        std::process::exit(1);
    });

    let download_url = if response.status() == reqwest::StatusCode::NOT_FOUND {
        println!(
            "Chrome version {chrome_version} not found in chrome-for-testing-public, \
             looking up nearest available version..."
        );
        let url = match find_chromedriver_url_from_api(&chrome_version) {
            Ok(url) => url,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };
        print!("Proceed with this version? [y/N] ");
        use std::io::Write as _;
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap_or(0);
        if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("Aborted.");
            std::process::exit(0);
        }
        url
    } else if !response.status().is_success() {
        eprintln!("Download failed with HTTP status {}.", response.status());
        std::process::exit(1);
    } else {
        // Direct URL worked — write the already-downloaded bytes
        let bytes = response.bytes().unwrap_or_else(|e| {
            eprintln!("Error reading response body: {e}");
            std::process::exit(1);
        });
        fs::write(&zip_path, &bytes).unwrap_or_else(|e| {
            eprintln!("Error writing zip file: {e}");
            std::process::exit(1);
        });
        println!("Downloaded {} bytes.", bytes.len());
        String::new() // sentinel: zip already written
    };

    // If we fell back to the API URL, perform the download now
    if !download_url.is_empty() {
        println!("Downloading {download_url}");
        let response = reqwest::blocking::get(&download_url).unwrap_or_else(|e| {
            eprintln!("Error downloading chromedriver: {e}");
            std::process::exit(1);
        });
        if !response.status().is_success() {
            eprintln!("Download failed with HTTP status {}.", response.status());
            std::process::exit(1);
        }
        let bytes = response.bytes().unwrap_or_else(|e| {
            eprintln!("Error reading response body: {e}");
            std::process::exit(1);
        });
        fs::write(&zip_path, &bytes).unwrap_or_else(|e| {
            eprintln!("Error writing zip file: {e}");
            std::process::exit(1);
        });
        println!("Downloaded {} bytes.", bytes.len());
    }

    // Step 4: Extract zip
    println!("Extracting archive...");
    let zip_file = fs::File::open(&zip_path).unwrap_or_else(|e| {
        eprintln!("Error opening zip file: {e}");
        std::process::exit(1);
    });

    let mut archive = zip::ZipArchive::new(zip_file).unwrap_or_else(|e| {
        eprintln!("Error reading zip archive: {e}");
        std::process::exit(1);
    });

    archive.extract(tmp_dir.path()).unwrap_or_else(|e| {
        eprintln!("Error extracting zip archive: {e}");
        std::process::exit(1);
    });

    // Step 5: Stop running chromedriver processes
    let extracted_binary = tmp_dir.path().join("chromedriver-linux64/chromedriver");
    let dest = "/usr/local/bin/chromedriver";

    match Command::new("pkill").arg("-x").arg("chromedriver").status() {
        Ok(status) if status.success() => println!("Stopped running chromedriver processes."),
        _ => {}
    }

    // Step 6: Copy binary to /usr/local/bin/
    println!("Installing chromedriver to {dest}...");
    match fs::copy(&extracted_binary, dest) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            eprintln!("Error: Permission denied writing to {dest}.");
            eprintln!("Try running with: sudo {}", std::env::args().collect::<Vec<_>>().join(" "));
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error copying chromedriver to {dest}: {e}");
            std::process::exit(1);
        }
    }

    fs::set_permissions(dest, fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| {
        eprintln!("Error setting permissions on {dest}: {e}");
        std::process::exit(1);
    });

    // Step 7: Confirm
    println!("chromedriver installed successfully.");
}
