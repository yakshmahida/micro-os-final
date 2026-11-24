//! Tier 1 & Tier 2 E2E Tests for R1: Bare-Metal x86_64 Kernel & Bootable Image Pipeline.
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)

use std::fs;
use std::path::Path;
use super::common::*;

// ============================================================================
// Tier 1: Feature Coverage (Happy Path)
// ============================================================================

/// `TC-T1-01`: Toolchain Config Validity.
/// Validates channel "nightly", target "x86_64-unknown-none", and required components.
#[test]
fn t1_build_01_toolchain_config_validity() {
    let toolchain_path = Path::new("rust-toolchain.toml");
    assert!(toolchain_path.exists(), "rust-toolchain.toml must exist on disk");
    let content = fs::read_to_string(toolchain_path).expect("Failed to read rust-toolchain.toml");

    assert!(
        content.contains("nightly"),
        "rust-toolchain.toml must specify the nightly channel"
    );
    assert!(
        content.contains("x86_64-unknown-none"),
        "rust-toolchain.toml must specify target x86_64-unknown-none"
    );
    assert!(
        content.contains("rust-src"),
        "rust-toolchain.toml must include rust-src component"
    );
}

/// `TC-T1-02`: Cargo Bindeps Config.
/// Validates `.cargo/config.toml` contains `[unstable] bindeps = true`.
#[test]
fn t1_build_02_cargo_bindeps_config() {
    let config_path = Path::new(".cargo/config.toml");
    assert!(config_path.exists(), ".cargo/config.toml must exist on disk");
    let content = fs::read_to_string(config_path).expect("Failed to read .cargo/config.toml");

    assert!(
        content.contains("bindeps"),
        ".cargo/config.toml must enable the bindeps feature"
    );
    assert!(
        content.contains("true"),
        ".cargo/config.toml bindeps must be set to true"
    );
}

/// `TC-T1-03`: Workspace Manifest Structure.
/// Validates root `Cargo.toml` declares workspace members including "kernel".
#[test]
fn t1_build_03_workspace_manifest_structure() {
    let cargo_path = Path::new("Cargo.toml");
    assert!(cargo_path.exists(), "Cargo.toml must exist on disk");
    let content = fs::read_to_string(cargo_path).expect("Failed to read Cargo.toml");

    assert!(
        content.contains("workspace"),
        "Cargo.toml must define a workspace"
    );
    assert!(
        content.contains("kernel"),
        "Cargo.toml workspace must include member 'kernel'"
    );
}

/// `TC-T1-04`: ISO 9660 & El Torito Generation.
/// Validates `dist/micro-os.iso`, Sector 16 PVD "CD001", and Sector 17/19 Boot Catalog.
#[test]
fn t1_build_04_iso9660_and_el_torito_generation() {
    let iso_path = Path::new("dist/micro-os.iso");
    assert!(iso_path.exists(), "dist/micro-os.iso must exist on disk");
    let iso_data = fs::read(iso_path).expect("Failed to read dist/micro-os.iso");

    // Verify Sector 16 PVD
    assert!(
        verify_iso_pvd_sector16(&iso_data).is_ok(),
        "ISO 9660 Sector 16 PVD verification failed"
    );

    // Verify Sector 17 El Torito Boot Record
    let catalog_sector = verify_el_torito_sector17(&iso_data)
        .expect("El Torito Sector 17 verification failed");
    assert_eq!(
        catalog_sector, 19,
        "Boot catalog sector pointer must equal sector 19"
    );

    // Verify Sector 19 Boot Catalog
    assert!(
        verify_el_torito_boot_catalog(&iso_data, catalog_sector).is_ok(),
        "Boot Catalog Sector 19 verification failed"
    );
}

/// `TC-T1-05`: VirtualBox VDI Disk Header.
/// Validates VDI signature `<<< Oracle VM VirtualBox Disk Image >>>` and header fields.
#[test]
fn t1_build_05_vbox_vdi_disk_header() {
    let vdi_path = Path::new("dist/micro-os-bios.vdi");
    assert!(vdi_path.exists(), "dist/micro-os-bios.vdi must exist on disk");
    let vdi_data = fs::read(vdi_path).expect("Failed to read dist/micro-os-bios.vdi");

    assert!(
        verify_vdi_header(&vdi_data).is_ok(),
        "VDI Header verification failed"
    );
}

// ============================================================================
// Tier 2: Boundary & Corner Cases (Stress & Limits)
// ============================================================================

/// `TC-T2-01`: ISO Sector Alignment Padding.
/// Validates exact 2048-byte sector boundary alignment across multiple arbitrary payloads.
#[test]
fn t2_build_01_iso_sector_alignment_padding() {
    let test_sizes = [1usize, 512, 1024, 2047, 2048, 2049, 4096, 65535, 65536];
    for &raw_size in &test_sizes {
        let padded_size = raw_size.div_ceil(ISO_SECTOR_SIZE) * ISO_SECTOR_SIZE;
        assert_eq!(
            padded_size % ISO_SECTOR_SIZE,
            0,
            "Padded size {} must be an exact multiple of 2048",
            padded_size
        );
        assert!(
            padded_size >= raw_size,
            "Padded size must be greater than or equal to raw size"
        );
        assert!(
            padded_size - raw_size < ISO_SECTOR_SIZE,
            "Padding overhead must be strictly less than 1 sector"
        );
    }

    // Verify standard ISO on disk is multiple of 2048
    let iso_path = Path::new("dist/micro-os.iso");
    assert!(iso_path.exists(), "dist/micro-os.iso must exist on disk");
    let iso_data = fs::read(iso_path).expect("Failed to read dist/micro-os.iso");
    assert_eq!(
        iso_data.len() % ISO_SECTOR_SIZE,
        0,
        "Generated ISO length must be a multiple of 2048"
    );
}

/// `TC-T2-02`: Missing Kernel Binary Detection.
/// Validates that ISO synthesis logic detects missing kernel file and returns an explicit error.
#[test]
fn t2_build_02_missing_kernel_binary_detection() {
    let non_existent = Path::new("target/non_existent_kernel_binary");
    let result = check_kernel_binary_path(non_existent);
    assert_eq!(
        result,
        Err("KernelNotFound"),
        "Missing kernel binary must return KernelNotFound error"
    );
}

/// `TC-T2-03`: Zero-Length Kernel Binary Handling.
/// Validates that ISO synthesis rejects 0-byte kernel binaries.
#[test]
fn t2_build_03_zero_length_kernel_binary_handling() {
    assert_eq!(
        validate_kernel_binary_slice(&[]),
        Err("KernelZeroLength"),
        "0-byte kernel binary must be rejected with KernelZeroLength"
    );
    assert_eq!(
        validate_kernel_binary_slice(&[0u8; 100]),
        Err("KernelTooSmall"),
        "Kernel under 512 bytes must be rejected with KernelTooSmall"
    );
    assert!(
        validate_kernel_binary_slice(&[0u8; 1024]).is_ok(),
        "Valid kernel size must be accepted"
    );
}

/// `TC-T2-04`: Non-Nightly Rustc Rejection.
/// Validates that non-nightly rustc version strings are identified as incompatible.
#[test]
fn t2_build_04_non_nightly_rustc_rejection() {
    assert_eq!(
        validate_toolchain_channel("rustc 1.82.0 (f6e511eec 2024-10-15)"),
        Err("RequiresNightlyCompiler"),
        "Stable rustc must be rejected"
    );
    assert_eq!(
        validate_toolchain_channel("rustc 1.81.0-beta.2 (016503cbf 2024-07-28)"),
        Err("RequiresNightlyCompiler"),
        "Beta rustc must be rejected"
    );
    assert!(
        validate_toolchain_channel("rustc 1.102.0-nightly (137d1d494 2026-11-22)").is_ok(),
        "Nightly rustc must be accepted"
    );
}

/// `TC-T2-05`: VBoxManage Missing Graceful Fallback.
/// Validates that lack of VBoxManage is handled safely with standard exit code 3 or warning.
#[test]
fn t2_build_05_vboxmanage_missing_graceful_fallback() {
    let res = evaluate_vboxmanage_status(false);
    assert!(res.is_err(), "Missing VBoxManage must result in Err");
    let (code, msg) = res.unwrap_err();
    assert_eq!(code, 3, "Exit code for missing prerequisite must be 3");
    assert!(
        msg.contains("VirtualBox"),
        "Error message must provide informative diagnostic"
    );
}
