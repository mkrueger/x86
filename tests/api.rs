use std::sync::{Arc, Mutex};
use x86::{
    ExecutionBackend, Image, ImageKind, Machine, MachineConfig, MachineResources, MachineStatus,
    Result, SavedState,
};

#[derive(Default)]
struct ObservedResources {
    bios: Option<(String, Vec<u8>)>,
    vga_bios: Option<(String, Vec<u8>)>,
    hard_disks: Vec<(String, Vec<u8>)>,
    cdroms: Vec<(String, Vec<u8>)>,
}

struct RecordingBackend(Arc<Mutex<ObservedResources>>);

impl ExecutionBackend for RecordingBackend {
    fn reset(&mut self, _config: &MachineConfig) -> Result<()> {
        Ok(())
    }

    fn prepare(
        &mut self,
        _config: &MachineConfig,
        resources: &MachineResources<'_>,
    ) -> Result<()> {
        let image = |image: &Image| (image.name().to_owned(), image.bytes().to_vec());
        let mut observed = self.0.lock().expect("resource recording lock");
        observed.bios = resources.bios.map(image);
        observed.vga_bios = resources.vga_bios.map(image);
        observed.hard_disks = resources.hard_disks.iter().map(image).collect();
        observed.cdroms = resources.cdroms.iter().map(image).collect();
        Ok(())
    }

    fn step(&mut self) -> Result<bool> {
        Ok(true)
    }

    fn read_memory(&self, _address: u64, _buffer: &mut [u8]) -> Result<()> {
        Ok(())
    }

    fn write_memory(&mut self, _address: u64, _data: &[u8]) -> Result<()> {
        Ok(())
    }
}

fn minimal_state() -> Vec<u8> {
    let metadata = br#"{"state":[65536],"buffer_infos":[]}"#;
    let total = 16 + metadata.len();
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(&0x8676_8676u32.to_le_bytes());
    bytes.extend_from_slice(&6u32.to_le_bytes());
    bytes.extend_from_slice(&(total as u32).to_le_bytes());
    bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    bytes.extend_from_slice(metadata);
    bytes
}

#[test]
fn image_reports_size_and_sha256() {
    let image = Image::from_bytes(ImageKind::RawDisk, "disk", vec![1, 2, 3]);
    assert_eq!(image.len(), 3);
    assert_eq!(
        image.sha256(),
        "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
    );
}

#[test]
fn state_header_and_summary_are_available() {
    let state = SavedState::from_bytes(minimal_state()).expect("valid state");
    assert_eq!(state.header().version, 6);
    assert_eq!(state.buffer_count(), 0);
    assert_eq!(state.memory_bytes(), Some(65536));
    assert!(!state.is_compressed());
}

#[cfg(feature = "zstd")]
#[test]
fn zstd_state_is_decoded() {
    let compressed = zstd::stream::encode_all(minimal_state().as_slice(), 1).expect("compress");
    let state = SavedState::from_bytes(compressed).expect("valid compressed state");
    assert!(state.is_compressed());
    assert_eq!(state.header().version, 6);
}

#[test]
fn machine_starts_without_backend() {
    let machine = Machine::new(MachineConfig::default().with_ram_bytes(64 * 1024 * 1024));
    assert_eq!(machine.status(), MachineStatus::Created);
    assert_eq!(machine.config().ram_bytes, 64 * 1024 * 1024);
}

#[test]
fn machine_passes_attached_resources_to_backend() {
    let observed = Arc::new(Mutex::new(ObservedResources::default()));
    let mut machine = Machine::new(MachineConfig::default());
    machine
        .set_bios(Image::from_bytes(ImageKind::Bios, "system.bin", [1, 2]))
        .expect("attach BIOS");
    machine
        .set_vga_bios(Image::from_bytes(ImageKind::VgaBios, "vga.bin", [3]))
        .expect("attach VGA BIOS");
    machine
        .set_disk(Image::from_bytes(ImageKind::RawDisk, "disk.img", [4, 5]))
        .expect("attach hard disk");
    machine
        .set_cdrom(Image::from_bytes(ImageKind::Iso9660, "disc.iso", [6]))
        .expect("attach CD-ROM");
    machine.attach_backend(RecordingBackend(Arc::clone(&observed)));

    machine.prepare().expect("prepare machine");

    let observed = observed.lock().expect("resource recording lock");
    assert_eq!(observed.bios, Some(("system.bin".to_owned(), vec![1, 2])));
    assert_eq!(observed.vga_bios, Some(("vga.bin".to_owned(), vec![3])));
    assert_eq!(
        observed.hard_disks,
        vec![("disk.img".to_owned(), vec![4, 5])]
    );
    assert_eq!(observed.cdroms, vec![("disc.iso".to_owned(), vec![6])]);
}
