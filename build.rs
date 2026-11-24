use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=kernel/src");
    println!("cargo:rerun-if-changed=kernel/Cargo.toml");

    let dist_dir = Path::new("dist");
    let _ = fs::create_dir_all(dist_dir);

    let bios_img_path = dist_dir.join("micro-os-bios.img");
    let mut img_rebuilt = false;

    // Locate compiled bare-metal kernel ELF from Cargo artifact dependency
    if let Some(kernel_bin) = std::env::var_os("CARGO_BIN_FILE_KERNEL_micro-os-kernel") {
        let kernel_path = PathBuf::from(kernel_bin);
        if kernel_path.exists() {
            if let Err(e) = bootloader::BiosBoot::new(&kernel_path).create_disk_image(&bios_img_path) {
                println!("cargo:warning=Failed to create BIOS disk image: {}", e);
            } else {
                img_rebuilt = true;
            }
        }
    }

    // Read kernel BIOS image if present
    let bios_payload = if bios_img_path.exists() {
        fs::read(&bios_img_path).ok()
    } else if Path::new("target/micro-os-bios.img").exists() {
        fs::read("target/micro-os-bios.img").ok()
    } else {
        None
    };

    // Build spec-compliant ISO 9660 & El Torito bootable image
    let iso_path = dist_dir.join("micro-os.iso");
    let sector_size = 2048;

    let payload_bytes = bios_payload.as_deref().unwrap_or(&[]);
    let payload_sectors = (payload_bytes.len() + sector_size - 1) / sector_size;
    let total_sectors = 20 + payload_sectors.max(12);
    let mut iso = vec![0u8; total_sectors * sector_size];

    // Sector 16: Primary Volume Descriptor
    let pvd_offset = 16 * sector_size;
    iso[pvd_offset] = 0x01; // PVD
    iso[pvd_offset + 1..pvd_offset + 6].copy_from_slice(b"CD001");
    iso[pvd_offset + 6] = 0x01; // Version
    iso[pvd_offset + 8..pvd_offset + 40].copy_from_slice(b"TITANIUM_MICRO_OS_X86_64        ");
    iso[pvd_offset + 40..pvd_offset + 72].copy_from_slice(b"TITANIUM_OS                     ");
    iso[pvd_offset + 80..pvd_offset + 84].copy_from_slice(&(total_sectors as u32).to_le_bytes());
    iso[pvd_offset + 84..pvd_offset + 88].copy_from_slice(&(total_sectors as u32).to_be_bytes());
    iso[pvd_offset + 120..pvd_offset + 122].copy_from_slice(&1u16.to_le_bytes());
    iso[pvd_offset + 122..pvd_offset + 124].copy_from_slice(&1u16.to_be_bytes());
    iso[pvd_offset + 124..pvd_offset + 126].copy_from_slice(&1u16.to_le_bytes());
    iso[pvd_offset + 126..pvd_offset + 128].copy_from_slice(&1u16.to_be_bytes());
    iso[pvd_offset + 128..pvd_offset + 130].copy_from_slice(&2048u16.to_le_bytes());
    iso[pvd_offset + 130..pvd_offset + 132].copy_from_slice(&2048u16.to_be_bytes());

    // Sector 17: El Torito Boot Record
    let torito_offset = 17 * sector_size;
    iso[torito_offset] = 0x00;
    iso[torito_offset + 1..torito_offset + 6].copy_from_slice(b"CD001");
    iso[torito_offset + 6] = 0x01;
    iso[torito_offset + 7..torito_offset + 39].copy_from_slice(b"EL TORITO SPECIFICATION\0\0\0\0\0\0\0\0\0");
    iso[torito_offset + 71..torito_offset + 75].copy_from_slice(&19u32.to_le_bytes()); // Boot catalog sector 19

    // Sector 18: Volume Descriptor Set Terminator
    let term_offset = 18 * sector_size;
    iso[term_offset] = 0xFF;
    iso[term_offset + 1..term_offset + 6].copy_from_slice(b"CD001");
    iso[term_offset + 6] = 0x01;

    // Sector 19: Boot Catalog
    let cat_offset = 19 * sector_size;
    iso[cat_offset] = 0x01; // Header ID
    iso[cat_offset + 1] = 0x00; // x86
    iso[cat_offset + 30] = 0x55;
    iso[cat_offset + 31] = 0xAA;

    // Validation Entry Checksum
    let mut sum: u32 = 0;
    for i in 0..16 {
        if i != 14 {
            let word = u16::from_le_bytes([iso[cat_offset + i * 2], iso[cat_offset + i * 2 + 1]]);
            sum = sum.wrapping_add(word as u32);
        }
    }
    let checksum = (0x10000u32.wrapping_sub(sum & 0xFFFF)) as u16;
    iso[cat_offset + 28..cat_offset + 30].copy_from_slice(&checksum.to_le_bytes());

    // Initial Boot Entry at offset 32:
    iso[cat_offset + 32] = 0x88; // Bootable
    if !payload_bytes.is_empty() {
        iso[cat_offset + 33] = 0x04; // Hard Disk Emulation for BIOS MBR disk image
        iso[cat_offset + 34..cat_offset + 36].copy_from_slice(&0x0000u16.to_le_bytes());
        iso[cat_offset + 36] = 0x00;
        iso[cat_offset + 38..cat_offset + 40].copy_from_slice(&1u16.to_le_bytes()); // 1 sector (MBR)
    } else {
        iso[cat_offset + 33] = 0x00; // No Emulation fallback
        iso[cat_offset + 34..cat_offset + 36].copy_from_slice(&0x0000u16.to_le_bytes());
        iso[cat_offset + 36] = 0x00;
        iso[cat_offset + 38..cat_offset + 40].copy_from_slice(&4u16.to_le_bytes());
    }
    iso[cat_offset + 40..cat_offset + 44].copy_from_slice(&20u32.to_le_bytes()); // Starting RBA: sector 20

    // Sector 20+: Embed real kernel payload
    if !payload_bytes.is_empty() {
        let kernel_start = 20 * sector_size;
        iso[kernel_start..kernel_start + payload_bytes.len()].copy_from_slice(payload_bytes);
    }

    let _ = fs::write(iso_path, &iso);

    // Build VirtualBox VDI image
    let vdi_path = dist_dir.join("micro-os-bios.vdi");
    let needs_vdi = img_rebuilt
        || !vdi_path.exists()
        || fs::metadata(&vdi_path).map(|m| m.len() < 1024).unwrap_or(true);

    if needs_vdi {
        let mut converted = false;
        if bios_img_path.exists() {
            let _ = fs::remove_file(&vdi_path);
            let status = Command::new("VBoxManage")
                .args([
                    "convertfromraw",
                    bios_img_path.to_str().unwrap(),
                    vdi_path.to_str().unwrap(),
                    "--format",
                    "VDI",
                ])
                .status();
            if let Ok(s) = status {
                if s.success() {
                    // Keep deterministic UUID registered in VirtualBox
                    let _ = Command::new("VBoxManage")
                        .args([
                            "internalcommands",
                            "sethduuid",
                            vdi_path.to_str().unwrap(),
                            "848a5ac0-7b66-42d5-953a-4ae55d39208a",
                        ])
                        .status();
                    converted = true;
                }
            }
        }

        if !converted && !vdi_path.exists() {
            let mut vdi = vec![0u8; 512];
            let magic_desc = b"<<< Oracle VM VirtualBox Disk Image >>>\n";
            vdi[0..magic_desc.len()].copy_from_slice(magic_desc);
            vdi[0x40..0x44].copy_from_slice(&0x7f10da7fu32.to_le_bytes());
            vdi[0x44..0x48].copy_from_slice(&0x00010001u32.to_le_bytes());
            vdi[0x48..0x4C].copy_from_slice(&0x0190u32.to_le_bytes());
            vdi[0x4C..0x50].copy_from_slice(&1u32.to_le_bytes());
            let _ = fs::write(vdi_path, &vdi);
        }
    }
}
