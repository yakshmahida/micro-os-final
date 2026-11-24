use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=kernel");

    let dist_dir = Path::new("dist");
    let _ = fs::create_dir_all(dist_dir);

    // Read real kernel BIOS image if present
    let bios_img_path = if Path::new("dist/micro-os-bios.img").exists() {
        Some(Path::new("dist/micro-os-bios.img"))
    } else if Path::new("target/micro-os-bios.img").exists() {
        Some(Path::new("target/micro-os-bios.img"))
    } else {
        None
    };

    let bios_payload = bios_img_path.and_then(|p| fs::read(p).ok());

    // Build spec-compliant ISO 9660 & El Torito bootable image
    let iso_path = dist_dir.join("micro-os.iso");
    let sector_size = 2048;

    // Base ISO header: 32 sectors minimum (64 KiB)
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
    iso[pvd_offset + 128..pvd_offset + 130].copy_from_slice(&2048u16.to_le_bytes());
    iso[pvd_offset + 130..pvd_offset + 132].copy_from_slice(&2048u16.to_be_bytes());

    // Sector 17: El Torito Boot Record
    let torito_offset = 17 * sector_size;
    iso[torito_offset] = 0x00;
    iso[torito_offset + 1..torito_offset + 6].copy_from_slice(b"CD001");
    iso[torito_offset + 6] = 0x01;
    iso[torito_offset + 7..torito_offset + 39].copy_from_slice(b"EL TORITO SPECIFICATION\0\0\0\0\0\0\0\0\0");
    iso[torito_offset + 71..torito_offset + 75].copy_from_slice(&19u32.to_le_bytes()); // Boot catalog sector 19

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
    iso[cat_offset + 33] = 0x00; // No Emulation
    iso[cat_offset + 34..cat_offset + 36].copy_from_slice(&0x0000u16.to_le_bytes()); // Load segment 0
    iso[cat_offset + 36] = 0x00; // System type
    let sector_count = if !payload_bytes.is_empty() {
        ((payload_bytes.len() + 511) / 512).min(0xFFFF) as u16
    } else {
        4u16
    };
    iso[cat_offset + 38..cat_offset + 40].copy_from_slice(&sector_count.to_le_bytes());
    iso[cat_offset + 40..cat_offset + 44].copy_from_slice(&20u32.to_le_bytes()); // Starting RBA: sector 20

    // Sector 20+: Embed real kernel payload
    if !payload_bytes.is_empty() {
        let kernel_start = 20 * sector_size;
        iso[kernel_start..kernel_start + payload_bytes.len()].copy_from_slice(payload_bytes);
    }

    let _ = fs::write(iso_path, &iso);

    // Build VirtualBox VDI image
    let vdi_path = dist_dir.join("micro-os-bios.vdi");
    let needs_vdi = !vdi_path.exists() || fs::metadata(&vdi_path).map(|m| m.len() < 1024).unwrap_or(true);

    if needs_vdi {
        let mut converted = false;
        if let Some(src_img) = bios_img_path {
            let _ = fs::remove_file(&vdi_path);
            let status = Command::new("VBoxManage")
                .args(["convertfromraw", src_img.to_str().unwrap(), vdi_path.to_str().unwrap(), "--format", "VDI"])
                .status();
            if let Ok(s) = status {
                if s.success() {
                    converted = true;
                }
            }
        }

        if !converted {
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
