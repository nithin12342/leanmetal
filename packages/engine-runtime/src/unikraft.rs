#[derive(Clone, Debug)]
pub struct UnikraftTarget {
    pub image_name: &'static str,
    pub memory_mb: usize,
    pub enable_virtio_net: bool,
    pub enable_virtio_block: bool,
}

impl Default for UnikraftTarget {
    fn default() -> Self {
        Self {
            image_name: "edgeflag_engine.raw",
            memory_mb: 512,
            enable_virtio_net: true,
            enable_virtio_block: true,
        }
    }
}

impl UnikraftTarget {
    pub fn build_qemu_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        args.push("-kernel".to_string());
        args.push(self.image_name.to_string());
        args.push("-m".to_string());
        args.push(format!("{}M", self.memory_mb));
        if self.enable_virtio_net {
            args.push("-device".to_string());
            args.push("virtio-net-pci".to_string());
        }
        if self.enable_virtio_block {
            args.push("-device".to_string());
            args.push("virtio-blk-pci".to_string());
        }
        args
    }
}
