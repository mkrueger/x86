#![cfg(feature = "native-runtime")]

use std::path::PathBuf;

use x86::{ExecutionBackend, Image, ImageKind, MachineConfig, MachineResources, NativeBackend};

fn bios_images() -> (Image, Image) {
    let bios = PathBuf::from(std::env::var_os("X86_BIOS").expect("set X86_BIOS to seabios.bin"));
    let vga_bios =
        PathBuf::from(std::env::var_os("X86_VGA_BIOS").expect("set X86_VGA_BIOS to vgabios.bin"));
    (
        Image::from_file(ImageKind::Bios, bios).expect("load BIOS"),
        Image::from_file(ImageKind::VgaBios, vga_bios).expect("load VGA BIOS"),
    )
}

fn prepare_backend<'a>(bios: &'a Image, vga_bios: &'a Image, disk: &'a Image) -> NativeBackend {
    let config = MachineConfig::default()
        .with_ram_bytes(64 * 1024 * 1024)
        .with_vga_memory_bytes(2 * 1024 * 1024);
    let resources = MachineResources {
        bios: Some(bios),
        vga_bios: Some(vga_bios),
        hard_disks: std::slice::from_ref(disk),
        floppy_disks: &[],
        cdroms: &[],
    };
    let mut backend = NativeBackend::new();
    backend
        .prepare(&config, &resources)
        .expect("prepare machine");
    backend
}

#[test]
#[ignore = "requires X86_BIOS and X86_VGA_BIOS integration-test assets"]
fn seabios_boots_an_mbr_that_writes_com1() {
    let (bios, vga_bios) = bios_images();
    let mut disk = vec![0; 32 * 1024 * 1024];
    let program = [
        0xBA, 0xFB, 0x03, // mov dx, 03fbh
        0xB0, 0x03, // mov al, 03h
        0xEE, // out dx, al
        0xBA, 0xF8, 0x03, // mov dx, 03f8h
        0xB0, b'I', 0xEE, 0xB0, b'C', 0xEE, 0xB0, b'B', 0xEE, 0xF4, 0xEB,
        0xFD, // hlt; jmp hlt
    ];
    disk[..program.len()].copy_from_slice(&program);
    disk[510..512].copy_from_slice(&[0x55, 0xAA]);
    let disk = Image::from_bytes(ImageKind::RawDisk, "com1-mbr.img", disk);
    let mut backend = prepare_backend(&bios, &vga_bios, &disk);
    let mut output = [0; 16];

    for _ in 0..20_000 {
        backend.step().expect("run machine");
        let count = backend.serial_output(0, &mut output).expect("read COM1");
        if count > 0 {
            assert_eq!(&output[..count], b"ICB");
            return;
        }
    }
    let cpu = backend.cpu().expect("native CPU");
    let mut boot_memory = [0; 32];
    backend
        .read_memory(0x7C00, &mut boot_memory)
        .expect("read boot memory");
    panic!(
        "SeaBIOS did not boot the COM1 MBR: ip=0x{:08X}, instructions={}, ata_stats={:?}, boot_memory={boot_memory:02X?}",
        cpu.instruction_pointer(),
        cpu.instruction_counter(),
        cpu.ata_disk_stats(),
    );
}

#[test]
#[ignore = "requires X86_BIOS and X86_VGA_BIOS integration-test assets"]
fn seabios_int14_writes_com1() {
    let (bios, vga_bios) = bios_images();
    let mut disk = vec![0; 32 * 1024 * 1024];
    let program = [
        0x31, 0xD2, // xor dx, dx (COM1)
        0xB8, 0xE3, 0x00, // mov ax, 00e3h (9600 8N1)
        0xCD, 0x14, // int 14h initialize
        0xB8, b'I', 0x01, 0xCD, 0x14, 0xB8, b'1', 0x01, 0xCD, 0x14, 0xB8, b'4', 0x01, 0xCD, 0x14,
        0xF4, 0xEB, 0xFD,
    ];
    disk[..program.len()].copy_from_slice(&program);
    disk[510..512].copy_from_slice(&[0x55, 0xAA]);
    let disk = Image::from_bytes(ImageKind::RawDisk, "int14-mbr.img", disk);
    let mut backend = prepare_backend(&bios, &vga_bios, &disk);
    let mut output = [0; 16];
    for _ in 0..20_000 {
        backend.step().expect("run machine");
        let count = backend.serial_output(0, &mut output).expect("read COM1");
        if count > 0 {
            assert_eq!(&output[..count], b"I14");
            return;
        }
    }
    panic!("SeaBIOS INT 14h did not emit COM1 output");
}

#[test]
#[ignore = "requires X86_BIOS and X86_VGA_BIOS integration-test assets"]
fn real_mode_repe_cmpsb_matches_command_names() {
    let (bios, vga_bios) = bios_images();
    let mut disk = vec![0; 32 * 1024 * 1024];
    let program = [
        0x0E, 0x1F, 0x0E, 0x07, 0xFC, // push cs/pop ds; push cs/pop es; cld
        0xBE, 0x2D, 0x7C, // mov si, 7c2dh
        0xBF, 0x31, 0x7C, // mov di, 7c31h
        0xB9, 0x04, 0x00, // mov cx, 4
        0xF3, 0xA6, // repe cmpsb
        0x75, 0x0C, // jne BAD
        0xBA, 0xF8, 0x03, 0xB0, b'O', 0xEE, 0xB0, b'K', 0xEE, 0xF4, 0xEB, 0xFD, 0xBA, 0xF8, 0x03,
        0xB0, b'B', 0xEE, 0xB0, b'A', 0xEE, 0xB0, b'D', 0xEE, 0xF4, 0xEB, 0xFD, b'E', b'C', b'H',
        b'O', b'E', b'C', b'H', b'O',
    ];
    disk[..program.len()].copy_from_slice(&program);
    disk[510..512].copy_from_slice(&[0x55, 0xAA]);
    let disk = Image::from_bytes(ImageKind::RawDisk, "repe-cmpsb.img", disk);
    let mut backend = prepare_backend(&bios, &vga_bios, &disk);
    let mut output = [0; 16];
    for _ in 0..20_000 {
        backend.step().expect("run machine");
        let count = backend.serial_output(0, &mut output).expect("read COM1");
        if count > 0 {
            assert_eq!(&output[..count], b"OK");
            return;
        }
    }
    panic!("REPE CMPSB test did not emit output");
}

#[test]
#[ignore = "requires X86_BIOS and X86_VGA_BIOS integration-test assets"]
fn real_mode_indirect_far_call_returns() {
    let (bios, vga_bios) = bios_images();
    let mut disk = vec![0; 32 * 1024 * 1024];
    let program = [
        0x0E, 0x58, // push cs; pop ax
        0xA3, 0x18, 0x7C, // mov [7c18h], ax (pointer segment)
        0xFF, 0x1E, 0x16, 0x7C, // call far [7c16h]
        0xF4, 0xEB, 0xFD, // hlt loop after return
        0xBA, 0xF8, 0x03, 0xB0, b'O', 0xEE, 0xB0, b'K', 0xEE, 0xCB, // target; retf
        0x0C, 0x7C, 0x00, 0x00, // pointer: 0000:7c0c
    ];
    disk[..program.len()].copy_from_slice(&program);
    disk[510..512].copy_from_slice(&[0x55, 0xAA]);
    let disk = Image::from_bytes(ImageKind::RawDisk, "indirect-far-call.img", disk);
    let mut backend = prepare_backend(&bios, &vga_bios, &disk);
    let mut output = [0; 16];
    for _ in 0..20_000 {
        backend.step().expect("run machine");
        let count = backend.serial_output(0, &mut output).expect("read COM1");
        if count > 0 {
            assert_eq!(&output[..count], b"OK");
            return;
        }
    }
    panic!("indirect far call did not reach its target");
}

#[test]
#[ignore = "requires X86_BIOS and X86_VGA_BIOS integration-test assets"]
fn real_mode_x87_push_wraps_stack_top() {
    let (bios, vga_bios) = bios_images();
    let mut disk = vec![0; 32 * 1024 * 1024];
    let program = [
        0xDB, 0xE3, // finit (TOP = 0)
        0xD9, 0xE8, // fld1 (TOP wraps to 7)
        0xBA, 0xF8, 0x03, 0xB0, b'O', 0xEE, 0xB0, b'K', 0xEE,
        0xF4, 0xEB, 0xFD,
    ];
    disk[..program.len()].copy_from_slice(&program);
    disk[510..512].copy_from_slice(&[0x55, 0xAA]);
    let disk = Image::from_bytes(ImageKind::RawDisk, "x87-stack-wrap.img", disk);
    let mut backend = prepare_backend(&bios, &vga_bios, &disk);
    let mut output = [0; 16];
    for _ in 0..20_000 {
        backend.step().expect("run machine");
        let count = backend.serial_output(0, &mut output).expect("read COM1");
        if count > 0 {
            assert_eq!(&output[..count], b"OK");
            return;
        }
    }
    panic!("x87 stack-wrap test did not emit output");
}

#[test]
#[ignore = "requires X86_BIOS and X86_VGA_BIOS integration-test assets"]
fn real_mode_loopne_compares_nul_terminated_command_names() {
    let (bios, vga_bios) = bios_images();
    let mut disk = vec![0; 32 * 1024 * 1024];
    let program = [
        0x0E, 0x1F, 0x0E, 0x07, 0xFC, 0xBE, 0x31, 0x7C, 0xBF, 0x36, 0x7C, 0xB9, 0xFF, 0xFF, 0xAC,
        0xAE, 0x75, 0x10, 0x84, 0xC0, 0xE0, 0xF8, 0xBA, 0xF8, 0x03, 0xB0, b'O', 0xEE, 0xB0, b'K',
        0xEE, 0xF4, 0xEB, 0xFD, 0xBA, 0xF8, 0x03, 0xB0, b'B', 0xEE, 0xB0, b'A', 0xEE, 0xB0, b'D',
        0xEE, 0xF4, 0xEB, 0xFD, b'E', b'C', b'H', b'O', 0, b'E', b'C', b'H', b'O', 0,
    ];
    disk[..program.len()].copy_from_slice(&program);
    disk[510..512].copy_from_slice(&[0x55, 0xAA]);
    let disk = Image::from_bytes(ImageKind::RawDisk, "loopne-strcmp.img", disk);
    let mut backend = prepare_backend(&bios, &vga_bios, &disk);
    let mut output = [0; 16];
    for _ in 0..20_000 {
        backend.step().expect("run machine");
        let count = backend.serial_output(0, &mut output).expect("read COM1");
        if count > 0 {
            assert_eq!(&output[..count], b"OK");
            return;
        }
    }
    panic!("LOOPNE string comparison did not emit output");
}

#[test]
#[ignore = "requires X86_BIOS, X86_VGA_BIOS, and X86_DISK integration-test assets"]
fn seabios_reaches_the_freedos_partition_boot_record() {
    let (bios, vga_bios) = bios_images();
    let disk_path =
        PathBuf::from(std::env::var_os("X86_DISK").expect("set X86_DISK to a raw FreeDOS image"));
    let mut disk = std::fs::read(disk_path).expect("load disk");
    let partition_lba = u32::from_le_bytes(disk[454..458].try_into().unwrap()) as usize;
    let partition_start = partition_lba * 512;
    let original_target = 0x3Eusize;
    let marker = [
        0xBA, 0xFB, 0x03, 0xB0, 0x03, 0xEE, // configure 8N1
        0xBA, 0xF8, 0x03, 0xB0, b'P', 0xEE, 0xB0, b'B', 0xEE, 0xB0, b'R', 0xEE, 0xF4, 0xEB, 0xFD,
    ];
    disk[partition_start + original_target..partition_start + original_target + marker.len()]
        .copy_from_slice(&marker);

    let disk = Image::from_bytes(ImageKind::RawDisk, "freedos-pbr-marker.img", disk);
    let mut backend = prepare_backend(&bios, &vga_bios, &disk);
    let mut output = [0; 16];
    for _ in 0..20_000 {
        backend.step().expect("run machine");
        let count = backend.serial_output(0, &mut output).expect("read COM1");
        if count > 0 {
            assert!(output[..count].starts_with(b"PBR"));
            return;
        }
    }
    let cpu = backend.cpu().expect("native CPU");
    panic!(
        "SeaBIOS never reached the FreeDOS partition boot record: ip=0x{:08X}, instructions={}, ata_stats={:?}",
        cpu.instruction_pointer(),
        cpu.instruction_counter(),
        cpu.ata_disk_stats(),
    );
}

#[test]
#[ignore = "requires X86_BIOS, X86_VGA_BIOS, and X86_DISK integration-test assets"]
fn freedos_partition_loader_does_not_report_a_disk_error() {
    let (bios, vga_bios) = bios_images();
    let disk_path =
        PathBuf::from(std::env::var_os("X86_DISK").expect("set X86_DISK to a raw FreeDOS image"));
    let mut disk = std::fs::read(disk_path).expect("load disk");
    let partition_lba = u32::from_le_bytes(disk[454..458].try_into().unwrap()) as usize;
    let error_handler = partition_lba * 512 + 0x148;
    let trap = [
        0xBA, 0xF8, 0x03, 0xB0, b'E', 0xEE, 0xB0, b'R', 0xEE, 0xB0, b'R', 0xEE, 0xF4, 0xEB, 0xFD,
    ];
    disk[error_handler..error_handler + trap.len()].copy_from_slice(&trap);
    let disk = Image::from_bytes(ImageKind::RawDisk, "freedos-error-trap.img", disk);
    let mut backend = prepare_backend(&bios, &vga_bios, &disk);
    let mut output = [0; 16];
    for _ in 0..20_000 {
        backend.step().expect("run machine");
        let count = backend.serial_output(0, &mut output).expect("read COM1");
        assert_eq!(
            count,
            0,
            "FreeDOS partition loader entered its disk-error path: {:?}",
            &output[..count]
        );
    }
}

#[test]
#[ignore = "requires X86_BIOS, X86_VGA_BIOS, and X86_DISK integration-test assets"]
fn boots_seabios_from_a_raw_freedos_disk() {
    let (bios, vga_bios) = bios_images();
    let disk =
        PathBuf::from(std::env::var_os("X86_DISK").expect("set X86_DISK to a raw FreeDOS image"));
    let disk = Image::from_file(ImageKind::RawDisk, disk).expect("load disk");
    let mut backend = prepare_backend(&bios, &vga_bios, &disk);
    let mut entered_boot_memory = false;
    let mut serial_output = Vec::new();
    let mut serial_buffer = [0; 4096];
    let mut maximum_bios_ticks = 0u32;
    let expected_serial = std::env::var_os("X86_EXPECT_SERIAL");
    let max_steps = std::env::var("X86_MAX_STEPS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(40_000);
    for iteration in 0..max_steps {
        backend.step().expect("run machine");
        if iteration == 5_000 && std::env::var_os("X86_PRESS_ENTER").is_some() {
            backend.inject_text("\n").expect("press Enter during boot");
        }
        if iteration == max_steps * 3 / 4 {
            if let Some(command) = std::env::var_os("X86_BOOT_COMMAND") {
                backend
                    .inject_text(&format!("{}\n", command.to_string_lossy()))
                    .expect("type DOS diagnostic command");
            }
        }
        entered_boot_memory |= backend.cpu().expect("native CPU").instruction_pointer() < 0xE_0000;
        let count = backend
            .serial_output(0, &mut serial_buffer)
            .expect("read COM1");
        serial_output.extend_from_slice(&serial_buffer[..count]);
        if expected_serial.as_ref().is_some_and(|expected| {
            String::from_utf8_lossy(&serial_output).contains(expected.to_string_lossy().as_ref())
        }) {
            return;
        }
        let mut ticks = [0; 4];
        backend
            .read_memory(0x46C, &mut ticks)
            .expect("read BIOS timer ticks");
        maximum_bios_ticks = maximum_bios_ticks.max(u32::from_le_bytes(ticks));
    }

    let (_, _, text_plane) = backend.vga_text_snapshot().expect("VGA text plane");
    let text = String::from_utf8_lossy(&text_plane.iter().step_by(2).copied().collect::<Vec<_>>())
        .into_owned();
    let firmware_log = String::from_utf8_lossy(&backend.firmware_log()).into_owned();
    let cpu = backend.cpu().expect("native CPU");
    let instruction_pointer = cpu.instruction_pointer();
    if let Some(path) = std::env::var_os("X86_DUMP_MEMORY") {
        let mut memory = vec![0; 1024 * 1024];
        backend
            .read_memory(0, &mut memory)
            .expect("dump low memory");
        std::fs::write(path, memory).expect("write low-memory dump");
    }
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
    let cpu_exceptions = native_v86_core::native_runtime::cpu_exception_counts();
    let software_interrupts = native_v86_core::native_runtime::software_interrupt_counts();
    let dos_console =
        String::from_utf8_lossy(&native_v86_core::native_runtime::dos_console_output())
            .into_owned();
    let ata_stats = cpu.ata_disk_stats().expect("ATA disk statistics");
    let ata_commands = cpu.ata_command_counts();
    let uart = cpu.uart_diagnostics();
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
    if let Some(expected) = expected_serial {
        let serial_output = String::from_utf8_lossy(&serial_output);
        assert!(
            serial_output.contains(expected.to_string_lossy().as_ref()),
            "expected COM1 output {expected:?}, got {serial_output:?}; dos_console={dos_console:?}, entered_boot_memory={entered_boot_memory}, maximum_bios_ticks={maximum_bios_ticks}, ip=0x{instruction_pointer:08X}, halted={}, instructions={}, registers={registers:08X?}, instruction_bytes={instruction_bytes:02X?}, cpu_exceptions={cpu_exceptions:?}, software_interrupts={software_interrupts:02X?}, unknown_io={unknown_io:?}, ata_stats={ata_stats:?}, ata_commands={ata_commands:02X?}, uart={uart:?}",
            cpu.halted(),
            cpu.instruction_counter(),
            registers = cpu.general_registers(),
        );
    }
}
