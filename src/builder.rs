use alloc::vec::Vec;

use crate::{ProgramHeader, VERSION, VPT_MAGIC, VptHeader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramBuilder {
    pub name: Vec<u8>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VptBuilder {
    vendor_id: u32,
    programs: Vec<ProgramBuilder>,
}

impl ProgramBuilder {
    pub const fn base_size(&self) -> usize {
        size_of::<ProgramHeader>() + self.name.len() + self.payload.len()
    }

    pub const fn size(&self) -> usize {
        (self.base_size() + 7) & !7
    }

    pub const fn padding_bytes(&self) -> usize {
        self.size() - self.base_size()
    }
}

impl VptBuilder {
    pub const fn new(vendor_id: u32) -> Self {
        Self {
            vendor_id,
            programs: Vec::new(),
        }
    }

    pub fn add_program(&mut self, program: ProgramBuilder) {
        self.programs.push(program);
    }

    pub fn build(self) -> Vec<u8> {
        let total_size = size_of::<VptHeader>()
            + self
                .programs
                .iter()
                .map(ProgramBuilder::size)
                .sum::<usize>();

        let mut bytes = Vec::with_capacity(total_size);

        bytes.extend_from_slice(bytemuck::bytes_of(&VptHeader {
            magic: VPT_MAGIC,
            version: VERSION,
            vendor_id: self.vendor_id,
            size: total_size as u32,
            program_count: self.programs.len() as u32,
        }));

        for program in self.programs.iter() {
            bytes.extend_from_slice(bytemuck::bytes_of(&ProgramHeader {
                name_len: program.name.len() as u32,
                payload_len: program.payload.len() as u32,
            }));

            bytes.extend_from_slice(&program.payload);
            bytes.extend_from_slice(&program.name);

            bytes.extend(core::iter::repeat_n(0, program.padding_bytes()));
        }

        bytes
    }
}
