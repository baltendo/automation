use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

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
    let url = format!(
        "https://storage.googleapis.com/chrome-for-testing-public/{}/linux64/chromedriver-linux64.zip",
        chrome_version
    );
    println!("Downloading {url}");

    let tmp_dir = tempfile::tempdir().unwrap_or_else(|e| {
        eprintln!("Error creating temp directory: {e}");
        std::process::exit(1);
    });

    let zip_path = tmp_dir.path().join("chromedriver-linux64.zip");

    let response = reqwest::blocking::get(&url).unwrap_or_else(|e| {
        eprintln!("Error downloading chromedriver: {e}");
        std::process::exit(1);
    });

    if !response.status().is_success() {
        eprintln!(
            "Download failed with HTTP status {}. Is Chrome version {chrome_version} available for chrome-for-testing?",
            response.status()
        );
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

    // Step 5: Copy binary to /usr/local/bin/
    let extracted_binary = tmp_dir.path().join("chromedriver-linux64/chromedriver");
    let dest = "/usr/local/bin/chromedriver";

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

    // Step 6: Confirm
    println!("chromedriver {chrome_version} installed successfully.");
}
