#![cfg(feature = "native-runtime")]

use std::path::PathBuf;

use x86::{ExecutionBackend, Image, ImageKind, MachineConfig, MachineResources, NativeBackend};

#[test]
#[ignore = "requires X86_BIOS, X86_VGA_BIOS, and X86_DISK integration-test assets"]
fn boots_seabios_from_a_raw_freedos_disk() {
    let bios = PathBuf::from(std::env::var_os("X86_BIOS").expect("set X86_BIOS to seabios.bin"));
    let vga_bios =
        PathBuf::from(std::env::var_os("X86_VGA_BIOS").expect("set X86_VGA_BIOS to vgabios.bin"));
    let disk =
        PathBuf::from(std::env::var_os("X86_DISK").expect("set X86_DISK to a raw FreeDOS image"));
    let config = MachineConfig::default()
        .with_ram_bytes(64 * 1024 * 1024)
        .with_vga_memory_bytes(2 * 1024 * 1024);
    let bios = Image::from_file(ImageKind::Bios, bios).expect("load BIOS");
    let vga_bios = Image::from_file(ImageKind::VgaBios, vga_bios).expect("load VGA BIOS");
    let disk = Image::from_file(ImageKind::RawDisk, disk).expect("load disk");
    let resources = MachineResources {
        bios: Some(&bios),
        vga_bios: Some(&vga_bios),
        hard_disks: std::slice::from_ref(&disk),
        floppy_disks: &[],
        cdroms: &[],
    };
    let mut backend = NativeBackend::new();
    backend
        .prepare(&config, &resources)
        .expect("prepare machine");
    let mut entered_boot_memory = false;
    let mut serial_output = Vec::new();
    let mut serial_buffer = [0; 4096];
    for _ in 0..20_000 {
        backend.step().expect("run machine");
        entered_boot_memory |= backend.cpu().expect("native CPU").instruction_pointer() < 0xE_0000;
        let count = backend.serial_output(0, &mut serial_buffer).expect("read COM1");
        serial_output.extend_from_slice(&serial_buffer[..count]);
    }

    let (_, _, text_plane) = backend.vga_text_snapshot().expect("VGA text plane");
    let text = String::from_utf8_lossy(&text_plane.iter().step_by(2).copied().collect::<Vec<_>>())
        .into_owned();
    let firmware_log = String::from_utf8_lossy(&backend.firmware_log()).into_owned();
    let cpu = backend.cpu().expect("native CPU");
    let instruction_pointer = cpu.instruction_pointer();
    let mut instruction_bytes = [0; 32];
    backend
        .read_memory(
            instruction_pointer.saturating_sub(16) as u64,
            &mut instruction_bytes,
        )
        .expect("read current instructions");
    let mut unknown_io = native_v86_core::native_runtime::unknown_io_counts();
    unknown_io.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    unknown_io.truncate(16);
    let ata_stats = cpu.ata_disk_stats().expect("ATA disk statistics");
    let firmware_or_disk_boot = text.contains("SeaBIOS")
        || text.contains("FreeDOS")
        || firmware_log.contains("SeaBIOS")
        || ata_stats.0 > 0;
    assert!(
        firmware_or_disk_boot,
        "expected firmware or DOS disk boot; ip=0x{:08X}, halted={}, instructions={}, registers={registers:08X?}, instruction_bytes={instruction_bytes:02X?}, unknown_io={unknown_io:?}, ata_stats={ata_stats:?}, screen={text:?}, firmware_log={firmware_log:?}",
        instruction_pointer,
        cpu.halted(),
        cpu.instruction_counter(),
        registers = cpu.general_registers(),
        instruction_bytes = instruction_bytes,
    );
    if let Some(expected) = std::env::var_os("X86_EXPECT_SERIAL") {
        let serial_output = String::from_utf8_lossy(&serial_output);
        assert!(
            serial_output.contains(expected.to_string_lossy().as_ref()),
            "expected COM1 output {expected:?}, got {serial_output:?}; entered_boot_memory={entered_boot_memory}, ip=0x{instruction_pointer:08X}, instructions={}, ata_stats={ata_stats:?}",
            cpu.instruction_counter(),
        );
    }
}
