use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

use iced_x86::{
    Decoder, DecoderOptions, FlowControl, Instruction, InstructionInfoFactory, Mnemonic, OpAccess,
    OpKind, Register,
};

mod copy;

const MAX_FILE: usize = 64 * 1024 * 1024;
const MAX_CODE: usize = 16 * 1024 * 1024;
const MAX_SECTIONS: usize = 96;
const MAX_IMPORTS: usize = 4096;
const MAX_INSTRUCTIONS: usize = 512 * 1024;
const WINDOW: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryError {
    Malformed(&'static str),
    Unsupported(&'static str),
    NotFound,
    Ambiguous { candidates: usize },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(reason) => write!(f, "invalid proxy DLL: {reason}"),
            Self::Unsupported(reason) => write!(f, "unsupported proxy DLL: {reason}"),
            Self::NotFound => f.write_str("proxy DLL has no verified login-version patch"),
            Self::Ambiguous { candidates } => {
                write!(
                    f,
                    "proxy DLL has {candidates} possible login-version patches"
                )
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}

type Result<T> = std::result::Result<T, DiscoveryError>;

fn u16_at(bytes: &[u8], at: usize) -> Result<u16> {
    let end = at
        .checked_add(2)
        .ok_or(DiscoveryError::Malformed("offset overflow"))?;
    let data = bytes
        .get(at..end)
        .ok_or(DiscoveryError::Malformed("truncated data"))?;
    Ok(u16::from_le_bytes([data[0], data[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    let end = at
        .checked_add(4)
        .ok_or(DiscoveryError::Malformed("offset overflow"))?;
    let data = bytes
        .get(at..end)
        .ok_or(DiscoveryError::Malformed("truncated data"))?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Import {
    Protect,
    CurrentProcess,
    Flush,
}

struct Section {
    address: u32,
    extent: u32,
    raw: std::ops::Range<usize>,
    executable: bool,
    writable: bool,
}

struct Image<'a> {
    bytes: &'a [u8],
    sections: Vec<Section>,
    imports: HashMap<u32, Import>,
}

impl<'a> Image<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() > MAX_FILE {
            return Err(DiscoveryError::Unsupported(
                "image exceeds inspection limit",
            ));
        }
        if bytes.get(..2) != Some(b"MZ") {
            return Err(DiscoveryError::Malformed("missing DOS header"));
        }
        let pe = u32_at(bytes, 0x3c)? as usize;
        if bytes.get(pe..pe.saturating_add(4)) != Some(b"PE\0\0") {
            return Err(DiscoveryError::Malformed("missing PE header"));
        }
        if u16_at(bytes, pe + 4)? != 0x14c {
            return Err(DiscoveryError::Unsupported("expected 32-bit x86"));
        }
        let count = u16_at(bytes, pe + 6)? as usize;
        if count == 0 || count > MAX_SECTIONS {
            return Err(DiscoveryError::Malformed("invalid section count"));
        }
        let optional_size = u16_at(bytes, pe + 20)? as usize;
        let optional = pe + 24;
        if optional_size < 112 || u16_at(bytes, optional)? != 0x10b {
            return Err(DiscoveryError::Unsupported("expected PE32 optional header"));
        }
        let table = optional
            .checked_add(optional_size)
            .ok_or(DiscoveryError::Malformed("header overflow"))?;
        let table_end = table
            .checked_add(count * 40)
            .ok_or(DiscoveryError::Malformed("header overflow"))?;
        if table_end > bytes.len() {
            return Err(DiscoveryError::Malformed("truncated section table"));
        }
        let base = u32_at(bytes, optional + 28)?;
        let image_size = u32_at(bytes, optional + 56)?;
        if image_size == 0 || base.checked_add(image_size).is_none() {
            return Err(DiscoveryError::Malformed("invalid image address range"));
        }
        let mut sections: Vec<Section> = Vec::with_capacity(count);
        for n in 0..count {
            let at = table + n * 40;
            let virtual_size = u32_at(bytes, at + 8)?;
            let rva = u32_at(bytes, at + 12)?;
            let raw_size = u32_at(bytes, at + 16)? as usize;
            let raw_start = u32_at(bytes, at + 20)? as usize;
            let flags = u32_at(bytes, at + 36)?;
            let extent = virtual_size.max(raw_size as u32);
            let end = rva
                .checked_add(extent)
                .ok_or(DiscoveryError::Malformed("section address overflow"))?;
            if end > image_size {
                return Err(DiscoveryError::Malformed("section exceeds image"));
            }
            let raw_end = raw_start
                .checked_add(raw_size)
                .ok_or(DiscoveryError::Malformed("section offset overflow"))?;
            if raw_end > bytes.len() || (raw_size != 0 && raw_start < table_end) {
                return Err(DiscoveryError::Malformed("invalid section file range"));
            }
            let address = base + rva;
            for prior in &sections {
                if extent != 0
                    && prior.extent != 0
                    && address < prior.address + prior.extent
                    && prior.address < address + extent
                {
                    return Err(DiscoveryError::Malformed("overlapping virtual sections"));
                }
                if raw_size != 0
                    && !prior.raw.is_empty()
                    && raw_start < prior.raw.end
                    && prior.raw.start < raw_end
                {
                    return Err(DiscoveryError::Malformed("overlapping file sections"));
                }
            }
            sections.push(Section {
                address,
                extent,
                raw: raw_start..raw_end,
                executable: flags & 0x2000_0000 != 0,
                writable: flags & 0x8000_0000 != 0,
            });
        }
        let mut image = Self {
            bytes,
            sections,
            imports: HashMap::new(),
        };
        let directory_count = u32_at(bytes, optional + 92)? as usize;
        if directory_count > (optional_size - 96) / 8 {
            return Err(DiscoveryError::Malformed(
                "data directories exceed optional header",
            ));
        }
        if directory_count < 2 {
            return Ok(image);
        }
        let import_rva = u32_at(bytes, optional + 104)?;
        let import_size = u32_at(bytes, optional + 108)?;
        if import_rva == 0 && import_size == 0 {
            return Ok(image);
        }
        if import_rva == 0 || !(20..=20 * 256).contains(&import_size) {
            return Err(DiscoveryError::Malformed("invalid import directory"));
        }
        let import_address = base
            .checked_add(import_rva)
            .ok_or(DiscoveryError::Malformed("import address overflow"))?;
        let descriptors = image
            .data(import_address, import_size as usize)
            .ok_or(DiscoveryError::Malformed("unmapped import directory"))?;
        let mut terminated = false;
        let mut total = 0;
        let mut slots = HashSet::new();
        for descriptor in descriptors.chunks_exact(20) {
            if descriptor.iter().all(|&byte| byte == 0) {
                terminated = true;
                break;
            }
            let original = u32_at(descriptor, 0)?;
            let name = base
                .checked_add(u32_at(descriptor, 12)?)
                .ok_or(DiscoveryError::Malformed("import name overflow"))?;
            let first = u32_at(descriptor, 16)?;
            if first == 0 {
                return Err(DiscoveryError::Malformed("missing import address table"));
            }
            let library = image.string(name)?;
            let kernel = library.eq_ignore_ascii_case(b"kernel32.dll")
                || library.eq_ignore_ascii_case(b"kernelbase.dll");
            let lookup = if original == 0 { first } else { original };
            let mut index = 0u32;
            loop {
                total += 1;
                if total > MAX_IMPORTS {
                    return Err(DiscoveryError::Unsupported("too many imports"));
                }
                let offset = index
                    .checked_mul(4)
                    .ok_or(DiscoveryError::Malformed("import index overflow"))?;
                let lookup_address = base
                    .checked_add(lookup)
                    .and_then(|a| a.checked_add(offset))
                    .ok_or(DiscoveryError::Malformed("import table overflow"))?;
                let value = image
                    .word(lookup_address)
                    .ok_or(DiscoveryError::Malformed("truncated import lookup"))?;
                let slot = base
                    .checked_add(first)
                    .and_then(|a| a.checked_add(offset))
                    .ok_or(DiscoveryError::Malformed("import slot overflow"))?;
                if slot & 3 != 0 || !slots.insert(slot) {
                    return Err(DiscoveryError::Malformed(
                        "overlapping or unaligned import slots",
                    ));
                }
                if image.word(slot).is_none() {
                    return Err(DiscoveryError::Malformed("truncated import address table"));
                }
                if value == 0 {
                    break;
                }
                if value & 0x8000_0000 == 0 {
                    let name_address = base
                        .checked_add(value)
                        .and_then(|a| a.checked_add(2))
                        .ok_or(DiscoveryError::Malformed("import symbol overflow"))?;
                    let symbol = image.string(name_address)?;
                    let import = if kernel {
                        match symbol {
                            b"VirtualProtect" => Some(Import::Protect),
                            b"GetCurrentProcess" => Some(Import::CurrentProcess),
                            b"FlushInstructionCache" => Some(Import::Flush),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(import) = import {
                        image.imports.insert(slot, import);
                    }
                }
                index += 1;
            }
        }
        if !terminated {
            return Err(DiscoveryError::Malformed("unterminated import directory"));
        }
        Ok(image)
    }

    fn section(&self, address: u32, length: usize) -> Option<&Section> {
        let end = u64::from(address).checked_add(length as u64)?;
        self.sections.iter().find(|section| {
            address >= section.address
                && end <= u64::from(section.address) + u64::from(section.extent)
        })
    }

    fn data(&self, address: u32, length: usize) -> Option<&'a [u8]> {
        let section = self.section(address, length)?;
        let offset = (address - section.address) as usize;
        let end = offset.checked_add(length)?;
        if end > section.raw.len() {
            return None;
        }
        self.bytes
            .get(section.raw.start + offset..section.raw.start + end)
    }

    fn word(&self, address: u32) -> Option<u32> {
        let bytes = self.data(address, 4)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn string(&self, address: u32) -> Result<&'a [u8]> {
        let section = self
            .section(address, 1)
            .ok_or(DiscoveryError::Malformed("unmapped import string"))?;
        let offset = (address - section.address) as usize;
        let available = section
            .raw
            .len()
            .checked_sub(offset)
            .ok_or(DiscoveryError::Malformed("unmapped import string"))?;
        let bytes = self
            .data(address, available.min(256))
            .ok_or(DiscoveryError::Malformed("truncated import string"))?;
        let end = bytes
            .iter()
            .position(|&byte| byte == 0)
            .ok_or(DiscoveryError::Malformed("unterminated import string"))?;
        Ok(&bytes[..end])
    }

    fn executable(&self, address: u32) -> bool {
        self.section(address, 1).is_some_and(|s| s.executable) && self.data(address, 1).is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Base {
    Stack,
    Frame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Slot {
    base: Base,
    offset: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Value {
    Unknown,
    Constant(u32),
    Stack(Slot),
    Global(u32),
    Imported(Import),
    Process,
    Input(u8),
    Protection,
}

impl Value {
    fn add(self, amount: i32) -> Self {
        match self {
            Self::Constant(value) => Self::Constant(value.wrapping_add(amount as u32)),
            Self::Stack(slot) => slot
                .offset
                .checked_add(amount)
                .map(|offset| Self::Stack(Slot { offset, ..slot }))
                .unwrap_or(Self::Unknown),
            _ if amount == 0 => self,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone)]
struct State {
    registers: [Value; 8],
    stack: HashMap<Slot, Value>,
    watched: Option<Slot>,
    changed: bool,
    zero: Option<bool>,
    external_write: bool,
}

fn register_index(register: Register) -> Option<usize> {
    match register.full_register32() {
        Register::EAX => Some(0),
        Register::ECX => Some(1),
        Register::EDX => Some(2),
        Register::EBX => Some(3),
        Register::ESP => Some(4),
        Register::EBP => Some(5),
        Register::ESI => Some(6),
        Register::EDI => Some(7),
        _ => None,
    }
}

fn writes(access: OpAccess) -> bool {
    matches!(
        access,
        OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite | OpAccess::ReadCondWrite
    )
}

impl State {
    fn new() -> Self {
        let mut registers = [Value::Unknown; 8];
        registers[4] = Value::Stack(Slot {
            base: Base::Stack,
            offset: 0,
        });
        registers[5] = Value::Stack(Slot {
            base: Base::Frame,
            offset: 0,
        });
        Self {
            registers,
            stack: HashMap::new(),
            watched: None,
            changed: false,
            zero: None,
            external_write: false,
        }
    }

    fn register(&self, register: Register) -> Value {
        if register.size() != 4 {
            return Value::Unknown;
        }
        register_index(register)
            .map(|n| self.registers[n])
            .unwrap_or(Value::Unknown)
    }

    fn set_register(&mut self, register: Register, value: Value) {
        if let Some(n) = register_index(register) {
            self.registers[n] = if register.size() == 4 {
                value
            } else {
                Value::Unknown
            };
        }
    }

    fn address(&self, instruction: &Instruction) -> Value {
        if matches!(instruction.segment_prefix(), Register::FS | Register::GS) {
            return Value::Unknown;
        }
        let base = match instruction.memory_base() {
            Register::None => Value::Constant(0),
            register => self.register(register),
        };
        let index = match instruction.memory_index() {
            Register::None => 0,
            register => match self.register(register) {
                Value::Constant(value) => value.wrapping_mul(instruction.memory_index_scale()),
                _ => return Value::Unknown,
            },
        };
        base.add(instruction.memory_displacement32().wrapping_add(index) as i32)
    }

    fn load(&self, address: Value, image: &Image<'_>) -> Value {
        match address {
            Value::Stack(slot) => self.stack.get(&slot).copied().unwrap_or(Value::Unknown),
            Value::Constant(address) => {
                if let Some(&import) = image.imports.get(&address) {
                    return Value::Imported(import);
                }
                match image.section(address, 4) {
                    Some(section) if section.writable => Value::Global(address),
                    Some(_) => image
                        .word(address)
                        .map(Value::Constant)
                        .unwrap_or(Value::Unknown),
                    None => Value::Unknown,
                }
            }
            _ => Value::Unknown,
        }
    }

    fn operand(&self, instruction: &Instruction, n: u32, image: &Image<'_>) -> Value {
        match instruction.op_kind(n) {
            OpKind::Register => self.register(instruction.op_register(n)),
            OpKind::Memory if instruction.memory_size().size() == 4 => {
                self.load(self.address(instruction), image)
            }
            OpKind::Immediate8
            | OpKind::Immediate16
            | OpKind::Immediate32
            | OpKind::Immediate8to32 => Value::Constant(instruction.immediate(n) as u32),
            _ => Value::Unknown,
        }
    }

    fn store(&mut self, address: Value, size: usize, value: Value) {
        if let Value::Stack(slot) = address {
            let overlaps = |other: Slot| {
                other.base == slot.base
                    && i64::from(other.offset) < i64::from(slot.offset) + size as i64
                    && i64::from(slot.offset) < i64::from(other.offset) + 4
            };
            if self.watched.is_some_and(overlaps) {
                self.changed = true;
            }
            self.stack.retain(|&other, _| !overlaps(other));
            if size == 4 {
                self.stack.insert(slot, value);
            }
        } else {
            self.stack.clear();
            self.external_write = true;
            if self.watched.is_some() {
                self.changed = true;
            }
        }
    }

    fn argument(&self, n: i32, image: &Image<'_>) -> Value {
        self.load(self.register(Register::ESP).add(n * 4), image)
    }

    fn pop_arguments(&mut self, bytes: i32) {
        self.set_register(Register::ESP, self.register(Register::ESP).add(bytes));
    }

    fn clobber(&mut self) {
        self.registers[..3].fill(Value::Unknown);
        self.zero = None;
    }

    fn merge(&mut self, other: &Self) {
        for (left, right) in self.registers.iter_mut().zip(other.registers) {
            if *left != right {
                *left = Value::Unknown;
            }
        }
        self.stack
            .retain(|slot, value| other.stack.get(slot) == Some(value));
        self.changed |= other.changed;
        self.external_write |= other.external_write;
        if self.zero != other.zero {
            self.zero = None;
        }
    }

    fn branch_taken(&self, instruction: &Instruction) -> Option<bool> {
        match instruction.mnemonic() {
            Mnemonic::Je => self.zero,
            Mnemonic::Jne => self.zero.map(|zero| !zero),
            _ => None,
        }
    }

    fn import(&self, instruction: &Instruction, image: &Image<'_>) -> Option<Import> {
        match self.operand(instruction, 0, image) {
            Value::Imported(import) => Some(import),
            _ => None,
        }
    }

    fn step(
        &mut self,
        instruction: &Instruction,
        image: &Image<'_>,
        info: &mut InstructionInfoFactory,
    ) {
        let register = instruction.op0_register();
        match instruction.mnemonic() {
            Mnemonic::Mov if instruction.op0_kind() == OpKind::Register => {
                let value = self.operand(instruction, 1, image);
                self.set_register(register, value);
                return;
            }
            Mnemonic::Mov if instruction.op0_kind() == OpKind::Memory => {
                let value = self.operand(instruction, 1, image);
                self.store(
                    self.address(instruction),
                    instruction.memory_size().size(),
                    value,
                );
                return;
            }
            Mnemonic::Lea => {
                self.set_register(register, self.address(instruction));
                return;
            }
            Mnemonic::Push => {
                if instruction.stack_pointer_increment() != -4 {
                    self.stack.clear();
                    self.set_register(Register::ESP, Value::Unknown);
                    self.changed = true;
                    return;
                }
                let value = self.operand(instruction, 0, image);
                let stack = self.register(Register::ESP).add(-4);
                self.set_register(Register::ESP, stack);
                self.store(stack, 4, value);
                return;
            }
            Mnemonic::Pop if instruction.op0_kind() == OpKind::Register => {
                if instruction.stack_pointer_increment() != 4 {
                    self.stack.clear();
                    self.set_register(Register::ESP, Value::Unknown);
                    self.changed = true;
                    return;
                }
                let value = self.argument(0, image);
                self.pop_arguments(4);
                self.set_register(register, value);
                return;
            }
            Mnemonic::Add | Mnemonic::Sub if instruction.op0_kind() == OpKind::Register => {
                if let Value::Constant(amount) = self.operand(instruction, 1, image) {
                    let amount = if instruction.mnemonic() == Mnemonic::Sub {
                        amount.wrapping_neg()
                    } else {
                        amount
                    };
                    let value = self.register(register).add(amount as i32);
                    self.set_register(register, value);
                    self.zero = match value {
                        Value::Constant(value) => Some(value == 0),
                        _ => None,
                    };
                    return;
                }
            }
            Mnemonic::Xor
                if instruction.op0_kind() == OpKind::Register
                    && instruction.op1_kind() == OpKind::Register
                    && register == instruction.op1_register() =>
            {
                self.set_register(register, Value::Constant(0));
                self.zero = Some(true);
                return;
            }
            Mnemonic::Cmp | Mnemonic::Test => {
                self.zero = match (
                    self.operand(instruction, 0, image),
                    self.operand(instruction, 1, image),
                ) {
                    (Value::Constant(left), Value::Constant(right)) => {
                        Some(if instruction.mnemonic() == Mnemonic::Cmp {
                            left == right
                        } else {
                            left & right == 0
                        })
                    }
                    _ => None,
                };
                return;
            }
            _ => {}
        }
        if instruction.rflags_modified() != 0 {
            self.zero = None;
        }
        let details = info.info(instruction);
        if details.used_memory().iter().any(|m| writes(m.access())) {
            self.store(
                self.address(instruction),
                instruction.memory_size().size(),
                Value::Unknown,
            );
        }
        for used in details.used_registers() {
            if writes(used.access()) {
                self.set_register(used.register(), Value::Unknown);
            }
        }
    }
}

fn direct_call(instruction: &Instruction) -> Option<u32> {
    (instruction.mnemonic() == Mnemonic::Call && instruction.op0_kind() == OpKind::NearBranch32)
        .then(|| instruction.near_branch32())
}

fn direct_import(instruction: &Instruction, image: &Image<'_>) -> Option<Import> {
    if instruction.mnemonic() != Mnemonic::Call {
        return None;
    }
    State::new().import(instruction, image)
}

fn return_guard(image: &Image<'_>, address: u32) -> bool {
    if !image.executable(address) {
        return false;
    }
    let Some(bytes) = image.data(address, 24) else {
        return false;
    };
    let instructions: Vec<_> =
        Decoder::with_ip(32, bytes, u64::from(address), DecoderOptions::NONE)
            .into_iter()
            .take(4)
            .collect();
    let [compare, branch, ret, failure] = instructions.as_slice() else {
        return false;
    };
    compare.mnemonic() == Mnemonic::Cmp
        && compare.op0_kind() == OpKind::Register
        && compare.op0_register().size() == 4
        && compare.op1_kind() == OpKind::Memory
        && matches!(State::new().operand(compare, 1, image), Value::Global(_))
        && branch.mnemonic() == Mnemonic::Jne
        && branch.near_branch32() == failure.ip32()
        && ret.mnemonic() == Mnemonic::Ret
        && ret.stack_pointer_increment() == 4
        && failure.flow_control() == FlowControl::UnconditionalBranch
        && image.executable(failure.near_branch32())
}

fn helper(image: &Image<'_>, address: u32) -> bool {
    let Some(section) = image.section(address, 1).filter(|s| s.executable) else {
        return false;
    };
    let available = section
        .raw
        .len()
        .saturating_sub((address - section.address) as usize)
        .min(1024);
    let Some(bytes) = image.data(address, available) else {
        return false;
    };
    let instructions: Vec<_> =
        Decoder::with_ip(32, bytes, u64::from(address), DecoderOptions::NONE)
            .into_iter()
            .take(256)
            .collect();
    let positions: HashMap<_, _> = instructions
        .iter()
        .enumerate()
        .map(|(n, i)| (i.ip32(), n))
        .collect();
    let mut initial = State::new();
    initial.set_register(Register::ECX, Value::Input(0));
    initial.set_register(Register::EDX, Value::Input(1));
    let preserved = [Register::EBX, Register::EBP, Register::ESI, Register::EDI];
    for (index, register) in preserved.iter().enumerate() {
        initial.set_register(*register, Value::Input(3 + index as u8));
    }
    initial.stack.insert(
        Slot {
            base: Base::Stack,
            offset: 4,
        },
        Value::Input(2),
    );
    let mut queue = VecDeque::from([(0usize, initial, 0u8, 0usize)]);
    let mut info = InstructionInfoFactory::new();
    let mut steps = 0;
    let mut verified = false;
    while let Some((mut at, mut state, mut phase, mut path_steps)) = queue.pop_front() {
        while let Some(instruction) = instructions.get(at) {
            steps += 1;
            path_steps += 1;
            if steps > 2048 || path_steps > 256 || queue.len() > 32 {
                return false;
            }
            if instruction.is_invalid() {
                break;
            }
            match instruction.flow_control() {
                FlowControl::ConditionalBranch => {
                    if !matches!(instruction.mnemonic(), Mnemonic::Je | Mnemonic::Jne) {
                        break;
                    }
                    let Some(&target) = positions.get(&instruction.near_branch32()) else {
                        break;
                    };
                    if target <= at {
                        break;
                    }
                    match state.branch_taken(instruction) {
                        Some(true) => {
                            at = target;
                            continue;
                        }
                        Some(false) => {}
                        None => queue.push_back((target, state.clone(), phase, path_steps)),
                    }
                }
                FlowControl::UnconditionalBranch => {
                    let Some(&target) = positions.get(&instruction.near_branch32()) else {
                        break;
                    };
                    if target <= at {
                        break;
                    }
                    at = target;
                    continue;
                }
                FlowControl::Return => {
                    if phase == 4 {
                        if instruction.stack_pointer_increment() != 4
                            || state.register(Register::ESP)
                                != Value::Stack(Slot {
                                    base: Base::Stack,
                                    offset: 0,
                                })
                            || preserved.iter().enumerate().any(|(index, register)| {
                                state.register(*register) != Value::Input(3 + index as u8)
                            })
                        {
                            return false;
                        }
                        verified = true;
                    }
                    break;
                }
                FlowControl::IndirectBranch | FlowControl::Exception | FlowControl::Interrupt => {
                    break;
                }
                _ => {}
            }
            if instruction.mnemonic() == Mnemonic::Call {
                match state.import(instruction, image) {
                    Some(Import::Protect) if phase == 0 || phase == 2 => {
                        let protection = if phase == 0 {
                            Value::Constant(0x40)
                        } else {
                            Value::Protection
                        };
                        if state.argument(0, image) != Value::Input(0)
                            || state.argument(1, image) != Value::Input(2)
                            || state.argument(2, image) != protection
                        {
                            break;
                        }
                        let output = state.argument(3, image);
                        if !matches!(output, Value::Stack(_)) {
                            break;
                        }
                        state.store(
                            output,
                            4,
                            if phase == 0 {
                                Value::Protection
                            } else {
                                Value::Unknown
                            },
                        );
                        state.pop_arguments(16);
                        state.clobber();
                        phase += 1;
                    }
                    Some(Import::CurrentProcess) if phase == 3 => {
                        state.clobber();
                        state.set_register(Register::EAX, Value::Process);
                    }
                    Some(Import::Flush) if phase == 3 => {
                        if state.argument(0, image) == Value::Process
                            && state.argument(1, image) == Value::Input(0)
                            && state.argument(2, image) == Value::Input(2)
                        {
                            state.pop_arguments(12);
                            state.clobber();
                            phase = 4;
                        } else {
                            break;
                        }
                    }
                    None if phase == 1
                        && direct_call(instruction)
                            .is_some_and(|target| image.executable(target)) =>
                    {
                        if state.argument(0, image) != Value::Input(0)
                            || state.argument(1, image) != Value::Input(1)
                            || state.argument(2, image) != Value::Input(2)
                        {
                            if !return_guard(image, direct_call(instruction).unwrap()) {
                                break;
                            }
                            state.clobber();
                        } else {
                            if !copy::verified(image, direct_call(instruction).unwrap()) {
                                return false;
                            }
                            state.clobber();
                            phase = 2;
                        }
                    }
                    None if phase == 4
                        && direct_call(instruction)
                            .is_some_and(|target| return_guard(image, target)) =>
                    {
                        state.clobber()
                    }
                    _ => break,
                }
            } else {
                state.step(instruction, image, &mut info);
                if state.external_write {
                    return false;
                }
            }
            at += 1;
        }
    }
    verified
}

fn candidate(
    image: &Image<'_>,
    instructions: &[Instruction],
    flush: usize,
    helper_cache: &mut HashMap<u32, bool>,
) -> Option<(u32, u32)> {
    let first = (flush.saturating_sub(24)..flush)
        .rev()
        .find(|&n| direct_call(&instructions[n]).is_some())?;
    let second = (flush + 1..instructions.len().min(flush + 25))
        .find(|&n| direct_call(&instructions[n]).is_some())?;
    let target = direct_call(&instructions[first])?;
    if direct_call(&instructions[second])? != target {
        return None;
    }
    if !*helper_cache
        .entry(target)
        .or_insert_with(|| helper(image, target))
    {
        return None;
    }
    let mut start = first.saturating_sub(WINDOW);
    for (n, instruction) in instructions.iter().enumerate().take(first).skip(start) {
        if instruction.is_invalid()
            || matches!(
                instruction.flow_control(),
                FlowControl::Return
                    | FlowControl::UnconditionalBranch
                    | FlowControl::IndirectBranch
                    | FlowControl::Exception
                    | FlowControl::Interrupt
            )
            || instruction.mnemonic() == Mnemonic::Call
        {
            start = n + 1;
        }
    }
    let mut state = State::new();
    let mut info = InstructionInfoFactory::new();
    let mut source = None;
    let mut destination = None;
    let mut word = None;
    let mut skipped_flush: Option<State> = None;
    for (n, instruction) in instructions.iter().enumerate().take(second + 1).skip(start) {
        if n == flush + 1
            && let Some(alternative) = skipped_flush.take()
        {
            state.merge(&alternative);
        }
        if instruction.is_invalid() {
            return None;
        }
        match instruction.flow_control() {
            FlowControl::Return
            | FlowControl::UnconditionalBranch
            | FlowControl::IndirectBranch
            | FlowControl::Exception
            | FlowControl::Interrupt => return None,
            FlowControl::ConditionalBranch if n < first => {
                if !matches!(instruction.mnemonic(), Mnemonic::Je | Mnemonic::Jne)
                    || instruction.near_branch32() <= instructions[second].ip32()
                    || state.branch_taken(instruction) == Some(true)
                {
                    return None;
                }
            }
            FlowControl::ConditionalBranch => {
                if !matches!(instruction.mnemonic(), Mnemonic::Je | Mnemonic::Jne)
                    || n >= flush
                    || instruction.near_branch32() != instructions[flush].next_ip32()
                {
                    return None;
                }
                match state.branch_taken(instruction) {
                    Some(true) => return None,
                    Some(false) => {}
                    None => match &mut skipped_flush {
                        Some(alternative) => alternative.merge(&state),
                        None => skipped_flush = Some(state.clone()),
                    },
                }
            }
            _ => {}
        }
        if n == first || n == second {
            let Value::Global(dst) = state.register(Register::ECX) else {
                return None;
            };
            let Value::Stack(src) = state.register(Register::EDX) else {
                return None;
            };
            let Value::Constant(value) = state.load(Value::Stack(src), image) else {
                return None;
            };
            if state.argument(0, image) != Value::Constant(4) {
                return None;
            }
            if n == first {
                source = Some(src);
                destination = Some(dst);
                word = Some(value);
                state.watched = Some(src);
                state.changed = false;
                state.clobber();
            } else if source == Some(src)
                && destination != Some(dst)
                && word == Some(value)
                && !state.changed
            {
                return Some((instructions[first].ip32(), value));
            } else {
                return None;
            }
        } else if instruction.mnemonic() == Mnemonic::Call {
            match state.import(instruction, image) {
                Some(Import::CurrentProcess) if n > first => {
                    state.clobber();
                    state.set_register(Register::EAX, Value::Process);
                }
                Some(Import::Flush) if n == flush => {
                    if state.argument(0, image) != Value::Process
                        || state.argument(1, image) != Value::Global(destination?)
                        || state.argument(2, image) != Value::Constant(4)
                    {
                        return None;
                    }
                    state.pop_arguments(12);
                    state.clobber();
                }
                _ => return None,
            }
        } else {
            state.step(instruction, image, &mut info);
        }
    }
    None
}

pub fn build_word(dll: &[u8]) -> Result<u32> {
    let image = Image::parse(dll)?;
    if !image
        .imports
        .values()
        .any(|&import| import == Import::Flush)
    {
        return Err(DiscoveryError::NotFound);
    }
    let code_size: usize = image
        .sections
        .iter()
        .filter(|s| s.executable)
        .map(|s| s.raw.len())
        .sum();
    if code_size > MAX_CODE {
        return Err(DiscoveryError::Unsupported(
            "executable sections exceed inspection limit",
        ));
    }
    let mut matches = HashSet::new();
    let mut helper_cache = HashMap::new();
    let mut decoded = 0usize;
    for section in image.sections.iter().filter(|s| s.executable) {
        let remaining = MAX_INSTRUCTIONS - decoded;
        let instructions: Vec<_> = Decoder::with_ip(
            32,
            &image.bytes[section.raw.clone()],
            u64::from(section.address),
            DecoderOptions::NONE,
        )
        .into_iter()
        .take(remaining + 1)
        .collect();
        if instructions.len() > remaining {
            return Err(DiscoveryError::Unsupported(
                "instruction count exceeds inspection limit",
            ));
        }
        decoded += instructions.len();
        for (n, instruction) in instructions.iter().enumerate() {
            if direct_import(instruction, &image) == Some(Import::Flush) {
                if let Some(found) = candidate(&image, &instructions, n, &mut helper_cache) {
                    matches.insert(found);
                }
                if helper_cache.len() > 512 {
                    return Err(DiscoveryError::Unsupported(
                        "too many possible patch helpers",
                    ));
                }
            }
        }
    }
    match matches.len() {
        0 => Err(DiscoveryError::NotFound),
        1 => Ok(matches.into_iter().next().unwrap().1),
        candidates => Err(DiscoveryError::Ambiguous { candidates }),
    }
}

#[cfg(test)]
mod tests;
