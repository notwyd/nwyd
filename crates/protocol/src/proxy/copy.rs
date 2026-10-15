use std::collections::{HashMap, VecDeque};

use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction, Mnemonic, OpKind, Register};

use super::{Image, register_index};

const MAX_DISTANCE: u32 = 8192;
const MAX_PATH: usize = 256;
const MAX_STEPS: usize = 2048;
const MAX_QUEUE: usize = 32;
const STACK_LIMIT: i32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Region {
    Source,
    Destination,
    Stack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Byte {
    Unknown,
    Constant(u8),
    Source(u8),
    Pointer(Region, i32, u8),
    Saved(u8, u8),
}

type Word = [Byte; 4];

fn constant(value: u32) -> Word {
    value.to_le_bytes().map(Byte::Constant)
}

fn number(value: Word) -> Option<u32> {
    let mut bytes = [0; 4];
    for (target, byte) in bytes.iter_mut().zip(value) {
        let Byte::Constant(value) = byte else {
            return None;
        };
        *target = value;
    }
    Some(u32::from_le_bytes(bytes))
}

fn pointer(region: Region, offset: i32) -> Word {
    std::array::from_fn(|n| Byte::Pointer(region, offset, n as u8))
}

fn address(value: Word) -> Option<(Region, i32)> {
    let Byte::Pointer(region, offset, 0) = value[0] else {
        return None;
    };
    (value == pointer(region, offset)).then_some((region, offset))
}

fn shifted(value: Word, amount: i32) -> Option<Word> {
    if let Some(value) = number(value) {
        return Some(constant(value.wrapping_add(amount as u32)));
    }
    let (region, offset) = address(value)?;
    Some(pointer(region, offset.checked_add(amount)?))
}

fn saved(register: usize) -> Word {
    std::array::from_fn(|n| Byte::Saved(register as u8, n as u8))
}

fn source() -> Word {
    std::array::from_fn(|n| Byte::Source(n as u8))
}

#[derive(Clone, Copy)]
enum Flag {
    Undefined,
    Unknown,
    Known(bool),
}

impl Flag {
    fn value(self) -> Option<Option<bool>> {
        match self {
            Self::Undefined => None,
            Self::Unknown => Some(None),
            Self::Known(value) => Some(Some(value)),
        }
    }
}

#[derive(Clone, Copy)]
struct Flags {
    zero: Flag,
    carry: Flag,
    sign: Flag,
    overflow: Flag,
    parity: Flag,
}

impl Flags {
    fn all(flag: Flag) -> Self {
        Self {
            zero: flag,
            carry: flag,
            sign: flag,
            overflow: flag,
            parity: flag,
        }
    }

    fn result(value: u32, carry: bool, overflow: bool) -> Self {
        Self {
            zero: Flag::Known(value == 0),
            carry: Flag::Known(carry),
            sign: Flag::Known(value >> 31 != 0),
            overflow: Flag::Known(overflow),
            parity: Flag::Known((value as u8).count_ones().is_multiple_of(2)),
        }
    }

    fn arithmetic(left: u32, right: u32, subtract: bool) -> Self {
        let (value, carry) = if subtract {
            left.overflowing_sub(right)
        } else {
            left.overflowing_add(right)
        };
        let overflow = if subtract {
            ((left ^ right) & (left ^ value)) >> 31 != 0
        } else {
            (!(left ^ right) & (left ^ value)) >> 31 != 0
        };
        Self::result(value, carry, overflow)
    }

    fn branch(self, mnemonic: Mnemonic) -> Option<Option<bool>> {
        fn pair(
            left: Flag,
            right: Flag,
            apply: impl Fn(bool, bool) -> bool,
        ) -> Option<Option<bool>> {
            Some(match (left.value()?, right.value()?) {
                (Some(left), Some(right)) => Some(apply(left, right)),
                _ => None,
            })
        }
        match mnemonic {
            Mnemonic::Je => self.zero.value(),
            Mnemonic::Jne => Some(self.zero.value()?.map(|v| !v)),
            Mnemonic::Jb => self.carry.value(),
            Mnemonic::Jae => Some(self.carry.value()?.map(|v| !v)),
            Mnemonic::Jbe => pair(self.carry, self.zero, |c, z| c || z),
            Mnemonic::Ja => pair(self.carry, self.zero, |c, z| !c && !z),
            Mnemonic::Js => self.sign.value(),
            Mnemonic::Jns => Some(self.sign.value()?.map(|v| !v)),
            Mnemonic::Jo => self.overflow.value(),
            Mnemonic::Jno => Some(self.overflow.value()?.map(|v| !v)),
            Mnemonic::Jp => self.parity.value(),
            Mnemonic::Jnp => Some(self.parity.value()?.map(|v| !v)),
            Mnemonic::Jl => pair(self.sign, self.overflow, |s, o| s != o),
            Mnemonic::Jge => pair(self.sign, self.overflow, |s, o| s == o),
            Mnemonic::Jle | Mnemonic::Jg => {
                let less = pair(self.sign, self.overflow, |s, o| s != o)?;
                Some(match (self.zero.value()?, less) {
                    (Some(zero), Some(less)) => Some(if mnemonic == Mnemonic::Jle {
                        zero || less
                    } else {
                        !zero && !less
                    }),
                    _ => None,
                })
            }
            _ => None,
        }
    }
}

#[derive(Clone)]
struct State {
    registers: [Word; 8],
    stack: HashMap<i32, Byte>,
    destination: Word,
    flags: Flags,
}

impl State {
    fn new() -> Self {
        let mut state = Self {
            registers: std::array::from_fn(saved),
            stack: HashMap::new(),
            destination: [Byte::Unknown; 4],
            flags: Flags::all(Flag::Undefined),
        };
        state.registers[4] = pointer(Region::Stack, 0);
        for (offset, value) in [
            (0, saved(8)),
            (4, pointer(Region::Destination, 0)),
            (8, pointer(Region::Source, 0)),
            (12, constant(4)),
        ] {
            for (n, byte) in value.into_iter().enumerate() {
                state.stack.insert(offset + n as i32, byte);
            }
        }
        state
    }

    fn register_part(register: Register) -> Option<(usize, usize, usize)> {
        let index = register_index(register)?;
        let size = register.size();
        if !matches!(size, 1 | 2 | 4) {
            return None;
        }
        let offset = usize::from(matches!(
            register,
            Register::AH | Register::CH | Register::DH | Register::BH
        ));
        Some((index, offset, size))
    }

    fn register(&self, register: Register) -> Option<Word> {
        let (index, offset, size) = Self::register_part(register)?;
        let mut value = constant(0);
        value[..size].copy_from_slice(&self.registers[index][offset..offset + size]);
        Some(value)
    }

    fn set_register(&mut self, register: Register, value: Word) -> Option<()> {
        let (index, offset, size) = Self::register_part(register)?;
        self.registers[index][offset..offset + size].copy_from_slice(&value[..size]);
        Some(())
    }

    fn memory_address(&self, instruction: &Instruction) -> Option<Word> {
        if !matches!(
            instruction.segment_prefix(),
            Register::None | Register::DS | Register::SS
        ) {
            return None;
        }
        let base = match instruction.memory_base() {
            Register::None => constant(0),
            register if register.size() == 4 => self.register(register)?,
            _ => return None,
        };
        let index = match instruction.memory_index() {
            Register::None => 0,
            register if register.size() == 4 => {
                number(self.register(register)?)?.wrapping_mul(instruction.memory_index_scale())
            }
            _ => return None,
        };
        shifted(
            base,
            instruction.memory_displacement32().wrapping_add(index) as i32,
        )
    }

    fn load(&self, location: Word, size: usize) -> Option<Word> {
        if !matches!(size, 1 | 2 | 4) {
            return None;
        }
        let (region, offset) = address(location)?;
        let end = offset.checked_add(size as i32)?;
        let mut value = constant(0);
        match region {
            Region::Source | Region::Destination if offset >= 0 && end <= 4 => {
                let memory = if region == Region::Source {
                    source()
                } else {
                    self.destination
                };
                value[..size].copy_from_slice(&memory[offset as usize..end as usize]);
            }
            Region::Stack if offset >= -STACK_LIMIT && end <= 16 => {
                for (n, byte) in value.iter_mut().take(size).enumerate() {
                    *byte = *self.stack.get(&(offset + n as i32))?;
                }
            }
            _ => return None,
        }
        Some(value)
    }

    fn store(&mut self, location: Word, size: usize, value: Word) -> Option<()> {
        if !matches!(size, 1 | 2 | 4) {
            return None;
        }
        let (region, offset) = address(location)?;
        let end = offset.checked_add(size as i32)?;
        match region {
            Region::Destination if offset >= 0 && end <= 4 => {
                self.destination[offset as usize..end as usize].copy_from_slice(&value[..size]);
            }
            Region::Stack if offset >= -STACK_LIMIT && end <= 0 => {
                for (n, byte) in value.into_iter().take(size).enumerate() {
                    self.stack.insert(offset + n as i32, byte);
                }
            }
            _ => return None,
        }
        Some(())
    }

    fn operand(&self, instruction: &Instruction, n: u32) -> Option<Word> {
        match instruction.op_kind(n) {
            OpKind::Register => self.register(instruction.op_register(n)),
            OpKind::Memory => self.load(
                self.memory_address(instruction)?,
                instruction.memory_size().size(),
            ),
            OpKind::Immediate8
            | OpKind::Immediate16
            | OpKind::Immediate32
            | OpKind::Immediate8to32 => Some(constant(instruction.immediate(n) as u32)),
            _ => None,
        }
    }

    fn assign(&mut self, instruction: &Instruction, value: Word) -> Option<()> {
        match instruction.op0_kind() {
            OpKind::Register => self.set_register(instruction.op0_register(), value),
            OpKind::Memory => self.store(
                self.memory_address(instruction)?,
                instruction.memory_size().size(),
                value,
            ),
            _ => None,
        }
    }

    fn returned(&self) -> bool {
        self.destination == source()
            && self.registers[4] == pointer(Region::Stack, 0)
            && [3, 5, 6, 7]
                .into_iter()
                .all(|n| self.registers[n] == saved(n))
            && self.load(pointer(Region::Stack, 0), 4) == Some(saved(8))
    }

    fn step(&mut self, instruction: &Instruction) -> Option<()> {
        let register = instruction.op0_register();
        match instruction.mnemonic() {
            Mnemonic::Nop => Some(()),
            Mnemonic::Mov => self.assign(instruction, self.operand(instruction, 1)?),
            Mnemonic::Lea if register.size() == 4 => {
                self.set_register(register, self.memory_address(instruction)?)
            }
            Mnemonic::Push if instruction.stack_pointer_increment() == -4 => {
                let value = self.operand(instruction, 0)?;
                let stack = shifted(self.registers[4], -4)?;
                if address(stack)?.0 != Region::Stack {
                    return None;
                }
                self.store(stack, 4, value)?;
                self.registers[4] = stack;
                Some(())
            }
            Mnemonic::Pop
                if instruction.op0_kind() == OpKind::Register
                    && instruction.stack_pointer_increment() == 4 =>
            {
                if address(self.registers[4])?.0 != Region::Stack {
                    return None;
                }
                let value = self.load(self.registers[4], 4)?;
                self.registers[4] = shifted(self.registers[4], 4)?;
                self.set_register(register, value)
            }
            Mnemonic::Cmp | Mnemonic::Test => {
                let size = match instruction.op0_kind() {
                    OpKind::Register => register.size(),
                    OpKind::Memory => instruction.memory_size().size(),
                    _ => return None,
                };
                if size != 4 {
                    return None;
                }
                let left = self.operand(instruction, 0)?;
                let right = self.operand(instruction, 1)?;
                self.flags = match (number(left), number(right)) {
                    (Some(left), Some(right)) if instruction.mnemonic() == Mnemonic::Cmp => {
                        Flags::arithmetic(left, right, true)
                    }
                    (Some(left), Some(right)) => Flags::result(left & right, false, false),
                    _ if instruction.mnemonic() == Mnemonic::Cmp
                        && left == right
                        && !left.contains(&Byte::Unknown) =>
                    {
                        Flags::result(0, false, false)
                    }
                    _ if instruction.mnemonic() == Mnemonic::Test => Flags {
                        carry: Flag::Known(false),
                        overflow: Flag::Known(false),
                        ..Flags::all(Flag::Unknown)
                    },
                    _ => Flags::all(Flag::Unknown),
                };
                Some(())
            }
            Mnemonic::Add | Mnemonic::Sub | Mnemonic::Inc | Mnemonic::Dec => {
                if instruction.op0_kind() != OpKind::Register || register.size() != 4 {
                    return None;
                }
                let left = self.operand(instruction, 0)?;
                let unary = matches!(instruction.mnemonic(), Mnemonic::Inc | Mnemonic::Dec);
                let right = if unary {
                    constant(1)
                } else {
                    self.operand(instruction, 1)?
                };
                let subtract = matches!(instruction.mnemonic(), Mnemonic::Sub | Mnemonic::Dec);
                let carry = self.flags.carry;
                self.flags = match (number(left), number(right)) {
                    (Some(left), Some(right)) => Flags::arithmetic(left, right, subtract),
                    _ => Flags::all(Flag::Unknown),
                };
                if unary {
                    self.flags.carry = carry;
                }
                let value = if let Some(right) = number(right) {
                    shifted(
                        left,
                        if subtract {
                            right.wrapping_neg()
                        } else {
                            right
                        } as i32,
                    )?
                } else if !subtract {
                    shifted(right, number(left)? as i32)?
                } else {
                    let (left_region, left_offset) = address(left)?;
                    let (right_region, right_offset) = address(right)?;
                    if left_region != right_region {
                        return None;
                    }
                    constant(left_offset.wrapping_sub(right_offset) as u32)
                };
                self.set_register(register, value)
            }
            Mnemonic::And | Mnemonic::Or | Mnemonic::Xor => {
                if instruction.op0_kind() != OpKind::Register || register.size() != 4 {
                    return None;
                }
                let left = self.operand(instruction, 0)?;
                let right = self.operand(instruction, 1)?;
                let value = if instruction.mnemonic() == Mnemonic::Xor
                    && instruction.op1_kind() == OpKind::Register
                    && register == instruction.op1_register()
                {
                    0
                } else {
                    let (left, right) = (number(left)?, number(right)?);
                    match instruction.mnemonic() {
                        Mnemonic::And => left & right,
                        Mnemonic::Or => left | right,
                        _ => left ^ right,
                    }
                };
                self.flags = Flags::result(value, false, false);
                self.set_register(register, constant(value))
            }
            Mnemonic::Shr | Mnemonic::Shl => {
                if instruction.op0_kind() != OpKind::Register || register.size() != 4 {
                    return None;
                }
                let count = number(self.operand(instruction, 1)?)? & 31;
                if count == 0 {
                    return Some(());
                }
                let value = number(self.operand(instruction, 0)?)?;
                let right = instruction.mnemonic() == Mnemonic::Shr;
                let result = if right {
                    value >> count
                } else {
                    value << count
                };
                let carry = if right {
                    (value >> (count - 1)) & 1 != 0
                } else {
                    (value >> (32 - count)) & 1 != 0
                };
                self.flags = Flags::result(result, carry, false);
                self.flags.overflow = if count == 1 {
                    Flag::Known(if right {
                        value >> 31 != 0
                    } else {
                        (result >> 31 != 0) ^ carry
                    })
                } else {
                    Flag::Undefined
                };
                self.set_register(register, constant(result))
            }
            _ => None,
        }
    }
}

pub(super) fn verified(image: &Image<'_>, entry: u32) -> bool {
    let Some(section) = image.section(entry, 1).filter(|section| section.executable) else {
        return false;
    };
    let mut queue = VecDeque::from([(entry, State::new(), 0usize)]);
    let mut steps = 0;
    let mut returned = false;
    while let Some((mut ip, mut state, mut path_steps)) = queue.pop_front() {
        loop {
            steps += 1;
            path_steps += 1;
            if steps > MAX_STEPS
                || path_steps > MAX_PATH
                || queue.len() > MAX_QUEUE
                || ip.abs_diff(entry) > MAX_DISTANCE
                || ip < section.address
            {
                return false;
            }
            let offset = (ip - section.address) as usize;
            let Some(available) = section.raw.len().checked_sub(offset) else {
                return false;
            };
            let Some(bytes) = image.data(ip, available.min(15)) else {
                return false;
            };
            let instruction =
                Decoder::with_ip(32, bytes, u64::from(ip), DecoderOptions::NONE).decode();
            if instruction.is_invalid()
                || instruction.has_lock_prefix()
                || instruction.has_rep_prefix()
                || instruction.has_repne_prefix()
            {
                return false;
            }
            match instruction.flow_control() {
                FlowControl::Return => {
                    if instruction.mnemonic() != Mnemonic::Ret
                        || instruction.stack_pointer_increment() != 4
                        || !state.returned()
                    {
                        return false;
                    }
                    returned = true;
                    break;
                }
                FlowControl::ConditionalBranch
                    if instruction.op0_kind() == OpKind::NearBranch32 =>
                {
                    let Some(taken) = state.flags.branch(instruction.mnemonic()) else {
                        return false;
                    };
                    match taken {
                        Some(true) => {
                            ip = instruction.near_branch32();
                            continue;
                        }
                        Some(false) => {}
                        None => queue.push_back((
                            instruction.near_branch32(),
                            state.clone(),
                            path_steps,
                        )),
                    }
                }
                FlowControl::UnconditionalBranch
                    if instruction.op0_kind() == OpKind::NearBranch32 =>
                {
                    ip = instruction.near_branch32();
                    continue;
                }
                FlowControl::Next if state.step(&instruction).is_some() => {}
                _ => return false,
            }
            ip = instruction.next_ip32();
        }
    }
    returned
}
