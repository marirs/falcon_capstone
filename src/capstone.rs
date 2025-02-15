extern crate libc;

pub use crate::capstone_sys::*;

use std::{cell::Cell, error::Error, ffi::CStr, fmt, mem::transmute};

/// Get the version of the capstone engine.
///
/// Returns major, minor and combined. combined is (major << 8 | minor), and it encodes both
/// major & minor versions.
///
/// # Examples
///
/// ```
/// use falcon_capstone::capstone as cs;
///
/// let (major, minor, combined) = cs::engine_version();
/// println!("Capstone version: {}.{}", major, minor);
/// assert_eq!(((major << 8) | minor) as u32, combined);
/// ```
pub fn engine_version() -> (i32, i32, u32) {
    let mut major: i32 = Default::default();
    let mut minor: i32 = Default::default();
    let combined;

    unsafe {
        combined = cs_version(&mut major, &mut minor);
    };

    (major, minor, combined)
}

/// Check if capstone supports an arch.
///
/// Returns `true` if `arch` is supported.
///
/// # Examples
///
/// ```
/// use falcon_capstone::capstone as cs;
///
/// let supported = if cs::support_arch(cs::cs_arch::CS_ARCH_ARM) { "is" } else { "isn't" };
/// println!("The ARM architecture {} supported!", supported);
/// ```
pub fn support_arch(arch: cs_arch) -> bool {
    unsafe { cs_support(arch as i32) }
}

/// Rust-friendly error wrapper over Capstone's low-level cs_err.
#[derive(Debug)]
pub struct CsErr {
    code: cs_err,
}

impl fmt::Display for CsErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let strerr;
        unsafe { strerr = CStr::from_ptr(cs_strerror(self.code)) };
        let strerr = strerr
            .to_str()
            .unwrap_or("Failed to creare the error message string");

        write!(f, "{}", strerr)
    }
}

impl Error for CsErr {
    fn description(&self) -> &str {
        let strerr;
        unsafe { strerr = CStr::from_ptr(cs_strerror(self.code)) };

        strerr
            .to_str()
            .unwrap_or("Failed to creare the error message string")
    }
}

impl CsErr {
    /// Create a Capstone error from a low-level cs_err code.
    pub fn new(code: cs_err) -> CsErr {
        // assert_ne!(code, cs_err::CS_ERR_OK);
        // cs_err can be CS_ERR_OK is there weren't enough bytes to disassemble
        // the instruction
        CsErr { code }
    }

    /// Get the low-level cr_err code.
    pub fn code(&self) -> cs_err {
        self.code
    }
}

/// Convert a cs_err to a Result<(), CsErr>.
fn to_res(code: cs_err) -> Result<(), CsErr> {
    if code != cs_err::CS_ERR_OK {
        Err(CsErr::new(code))
    } else {
        Ok(())
    }
}

/// Disassembled instruction.
///
/// A Rust-friendly struct to access fields of a disassembled instruction. This is a safe wrapper
/// over cs_insn.
#[derive(Debug)]
pub struct Instr {
    /// Instruction ID. Find the instruction id in the '{ARCH}_insn' enum in the header file of
    /// corresponding architecture.
    pub id: InstrIdArch,

    /// Address (EIP) of this instruction.
    pub address: u64,

    /// Size of this instruction.
    pub size: u16,

    /// Machine bytes of this instruction.
    pub bytes: Vec<u8>,

    /// Ascii text of instruction mnemonic.
    pub mnemonic: String,

    /// Ascii text of instruction operands.
    pub op_str: String,

    /// Detail of this instuction.
    pub detail: Option<Details>,
}

impl Instr {
    /// Create an `Instr` from a cs_insn.
    pub fn new(instr: &cs_insn, decode_detail: bool, arch: cs_arch) -> Instr {
        assert_ne!(arch, cs_arch::CS_ARCH_MAX);
        assert_ne!(arch, cs_arch::CS_ARCH_ALL);

        let id = unsafe {
            match arch {
                cs_arch::CS_ARCH_ARM => InstrIdArch::ARM(transmute::<u32, arm_insn>(instr.id)),
                cs_arch::CS_ARCH_ARM64 => {
                    InstrIdArch::ARM64(transmute::<u32, arm64_insn>(instr.id))
                }
                cs_arch::CS_ARCH_MIPS => InstrIdArch::MIPS(transmute::<u32, mips_insn>(instr.id)),
                cs_arch::CS_ARCH_X86 => InstrIdArch::X86(transmute::<u32, x86_insn>(instr.id)),
                cs_arch::CS_ARCH_PPC => InstrIdArch::PPC(transmute::<u32, ppc_insn>(instr.id)),
                cs_arch::CS_ARCH_SPARC => {
                    InstrIdArch::SPARC(transmute::<u32, sparc_insn>(instr.id))
                }
                cs_arch::CS_ARCH_SYSZ => InstrIdArch::SYSZ(transmute::<u32, sysz_insn>(instr.id)),
                cs_arch::CS_ARCH_XCORE => {
                    InstrIdArch::XCORE(transmute::<u32, xcore_insn>(instr.id))
                }

                #[cfg(feature = "capstone4")]
                cs_arch::CS_ARCH_M68K => InstrIdArch::M68K(transmute::<u32, m68k_insn>(instr.id)),
                #[cfg(feature = "capstone4")]
                cs_arch::CS_ARCH_TMS320C64X => {
                    InstrIdArch::TMS320C64X(transmute::<u32, tms320c64x_insn>(instr.id))
                }
                #[cfg(feature = "capstone4")]
                cs_arch::CS_ARCH_M680X => {
                    InstrIdArch::M680X(transmute::<u32, m680x_insn>(instr.id))
                }
                #[cfg(feature = "capstone4")]
                cs_arch::CS_ARCH_EVM => InstrIdArch::EVM(transmute::<u32, evm_insn>(instr.id)),
                _ => panic!("Unexpected arch: {:?}", arch),
            }
        };

        let mut bytes = Vec::new();
        for i in 0..instr.bytes.len() {
            bytes.push(instr.bytes[i]);
        }

        let mut mnemonic = String::new();
        for i in 0..instr.mnemonic.len() {
            if instr.mnemonic[i] == 0 {
                break;
            }
            mnemonic.push((instr.mnemonic[i] as u8) as char);
        }

        let mut op_str = String::new();
        for i in 0..instr.op_str.len() {
            if instr.op_str[i] == 0 {
                break;
            }
            op_str.push((instr.op_str[i] as u8) as char);
        }

        let detail = if decode_detail {
            let detail = unsafe { *instr.detail };
            let arch_union = detail.__bindgen_anon_1;

            let arch = unsafe {
                match arch {
                    cs_arch::CS_ARCH_ARM => DetailsArch::ARM(Box::new(arch_union.arm)),
                    cs_arch::CS_ARCH_ARM64 => DetailsArch::ARM64(arch_union.arm64),
                    cs_arch::CS_ARCH_MIPS => DetailsArch::MIPS(arch_union.mips),
                    cs_arch::CS_ARCH_X86 => DetailsArch::X86(arch_union.x86),
                    cs_arch::CS_ARCH_PPC => DetailsArch::PPC(arch_union.ppc),
                    cs_arch::CS_ARCH_SPARC => DetailsArch::SPARC(arch_union.sparc),
                    cs_arch::CS_ARCH_SYSZ => DetailsArch::SYSZ(arch_union.sysz),
                    cs_arch::CS_ARCH_XCORE => DetailsArch::XCORE(arch_union.xcore),

                    #[cfg(feature = "capstone4")]
                    cs_arch::CS_ARCH_M68K => DetailsArch::M68K(arch_union.m68k),
                    #[cfg(feature = "capstone4")]
                    cs_arch::CS_ARCH_TMS320C64X => DetailsArch::TMS320C64X(arch_union.tms320c64x),
                    #[cfg(feature = "capstone4")]
                    cs_arch::CS_ARCH_M680X => DetailsArch::M680X(arch_union.m680x),
                    #[cfg(feature = "capstone4")]
                    cs_arch::CS_ARCH_EVM => DetailsArch::EVM(arch_union.evm),
                    _ => panic!("Unexpected arch: {:?}", arch),
                }
            };

            let mut regs_read = Vec::new();
            for i in 0..detail.regs_read_count {
                regs_read.push(detail.regs_read[i as usize] as u32);
            }

            let mut regs_write = Vec::new();
            for i in 0..detail.regs_write_count {
                regs_write.push(detail.regs_write[i as usize] as u32);
            }

            let mut groups = Vec::new();
            for i in 0..detail.groups_count {
                groups.push(detail.groups[i as usize] as u32);
            }

            Some(Details {
                regs_read,
                regs_write,
                groups,
                arch,
            })
        } else {
            None
        };

        Instr {
            id,
            address: instr.address,
            size: instr.size,
            bytes,
            mnemonic,
            op_str,
            detail,
        }
    }
}

/// Architecture-specific instruction id.
///
/// # Examples
///
/// ```
/// use falcon_capstone::capstone as cs;
/// let code = vec![0x01, 0xc3]; // add ebx, eax
///
/// let dec = cs::Capstone::new(cs::cs_arch::CS_ARCH_X86, cs::CS_MODE_32).unwrap();
///
/// let buf = dec.disasm(code.as_slice(), 0, 0).unwrap();
/// let add = buf.get(0).unwrap();
/// if let cs::InstrIdArch::X86(insn) = add.id {
///     assert_eq!(insn, cs::x86_insn::X86_INS_ADD);
/// }
/// ```
#[derive(Debug, PartialEq)]
pub enum InstrIdArch {
    X86(x86_insn),
    ARM64(arm64_insn),
    ARM(arm_insn),
    MIPS(mips_insn),
    PPC(ppc_insn),
    SPARC(sparc_insn),
    SYSZ(sysz_insn),
    XCORE(xcore_insn),

    #[cfg(feature = "capstone4")]
    M68K(m68k_insn),
    #[cfg(feature = "capstone4")]
    TMS320C64X(tms320c64x_insn),
    #[cfg(feature = "capstone4")]
    M680X(m680x_insn),
    #[cfg(feature = "capstone4")]
    EVM(evm_insn),
}

/// Details of an instruction.
///
/// If CS_OPT_DETAIL is on, `Instr.detail` will be filled with this struct, that you can use to get
/// some details of an instruction (e.g. registers read, modified, ...).
///
/// # Examples
///
/// ```
/// use falcon_capstone::capstone as cs;
/// let code = vec![0x01, 0xc3]; // add ebx, eax
///
/// let dec = cs::Capstone::new(cs::cs_arch::CS_ARCH_X86, cs::CS_MODE_32).unwrap();
/// dec.option(cs::cs_opt_type::CS_OPT_DETAIL, cs::cs_opt_value::CS_OPT_ON).unwrap();
///
/// let buf = dec.disasm(code.as_slice(), 0, 0).unwrap();
/// let detail = buf.get(0).unwrap().detail.unwrap(); // `buf` contains only one 'add'.
/// assert_eq!(dec.reg_name(detail.regs_write[0]), Some("eflags"));
/// ```
#[derive(Debug)]
pub struct Details {
    /// List of implicit registers read by this insn.
    pub regs_read: Vec<u32>,

    /// List of implicit registers modified by this insn.
    pub regs_write: Vec<u32>,

    /// List of group this instruction belong to.
    pub groups: Vec<u32>,

    /// Architecture-specific details.
    pub arch: DetailsArch,
}

/// Architecture-specific part of detail.
#[derive(Debug)]
pub enum DetailsArch {
    X86(cs_x86),
    ARM64(cs_arm64),
    ARM(Box<cs_arm>),
    MIPS(cs_mips),
    PPC(cs_ppc),
    SPARC(cs_sparc),
    SYSZ(cs_sysz),
    XCORE(cs_xcore),

    #[cfg(feature = "capstone4")]
    M68K(cs_m68k),
    #[cfg(feature = "capstone4")]
    TMS320C64X(cs_tms320c64x),
    #[cfg(feature = "capstone4")]
    M680X(cs_m680x),
    #[cfg(feature = "capstone4")]
    EVM(cs_evm),
}

/// Buffer of disassembled instructions.
///
/// Provides a Rust-friendly interface to read the buffer of instructions disassembled by Capstone.
#[derive(Debug)]
pub struct InstrBuf {
    ptr: *mut cs_insn,
    count: usize,
    decode_detail: bool,
    arch: cs_arch,
}

impl Drop for InstrBuf {
    fn drop(&mut self) {
        unsafe {
            cs_free(self.ptr, self.count);
        }
    }
}

impl InstrBuf {
    /// Create an `InstrBuf` from a pointer to a cs_insn buffer. `count` is the number of
    /// instructions in `insn`. `decode_detail` states if details are available for the
    /// instructions in `insn`, if true `Instr` created by `get` will have `Details`. `arch` is the
    /// architecture to use to interpret the arch-specific part of cs_detail.
    pub fn new(insn: *mut cs_insn, count: usize, decode_detail: bool, arch: cs_arch) -> InstrBuf {
        InstrBuf {
            ptr: insn,
            count,
            decode_detail,
            arch,
        }
    }

    /// Get the number of instructions in this buffer.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Get the instruction at the requested index.
    pub fn get(&self, index: usize) -> Option<Instr> {
        if index >= self.count {
            return None;
        }

        let insn;

        // unsafe { insn = &(*(self.ptr.offset(index as isize))) }
        unsafe { insn = &(*(self.ptr.add(index))) }
        Some(Instr::new(insn, self.decode_detail, self.arch))
    }

    /// Create an iterator from the beginning of this buffer.
    pub fn iter(&self) -> InstrIter {
        InstrIter::new(self)
    }
}

/// Disassembled instructions iterator.
///
/// Iterate over the instructions of a buffer of disassembled instructions.
#[derive(Debug)]
pub struct InstrIter<'a> {
    buf: &'a InstrBuf,
    current: usize,
}

impl<'a> Iterator for InstrIter<'a> {
    type Item = Instr;

    fn next(&mut self) -> Option<Self::Item> {
        let instr = self.buf.get(self.current);
        self.current += 1;

        instr
    }
}

impl<'a> InstrIter<'a> {
    /// Create an `InstrIter` from the beginning of `buf`.
    pub fn new(buf: &InstrBuf) -> InstrIter {
        InstrIter { buf, current: 0 }
    }
}

/// Capstone handle.
#[derive(Debug)]
pub struct Capstone {
    handle: Cell<csh>,
    details_on: Cell<bool>,
    arch: cs_arch,
}

impl Drop for Capstone {
    fn drop(&mut self) {
        let err;

        unsafe {
            err = cs_close(self.handle.as_ptr());
        }

        if err != cs_err::CS_ERR_OK {
            panic!("{}", CsErr::new(err).to_string())
        }
    }
}

impl Capstone {
    /// Create a Capstone handle.
    ///
    /// `arch` architecture type (CS_ARCH_*), `mode` hardware mode (CS_MODE_*).
    pub fn new(arch: cs_arch, mode: cs_mode) -> Result<Capstone, CsErr> {
        let err;
        let mut handle = Default::default();

        unsafe { err = cs_open(arch, mode, &mut handle) };
        to_res(err)?;

        Ok(Capstone {
            handle: Cell::new(handle),
            details_on: Cell::new(false),
            arch,
        })
    }

    /// Set option for disassembling engine at runtime.
    ///
    /// `typ` type of option to set. `value` value of the option.
    pub fn option(&self, typ: cs_opt_type, value: cs_opt_value) -> Result<(), CsErr> {
        let err;

        unsafe {
            let value = transmute::<cs_opt_value, u32>(value) as usize;
            err = cs_option(self.handle.get(), typ, value);
        };
        to_res(err)?;

        // When an instruction is decoded we need to know if details are generated (to decide if
        // the `Details` struct must be created). Unfortunately Capstone doesn't provide a way to
        // get the value of an option, so we are forced to track the `DETAIL` option from here.
        if typ == cs_opt_type::CS_OPT_DETAIL {
            self.details_on.set(value == cs_opt_value::CS_OPT_ON);
        }

        Ok(())
    }

    /// Disassemble binary code, given the code buffer, address and number of instructions to be
    /// decoded.
    ///
    /// `buf` is the code buffer. `addr` is the address of the first instruction, `count` is the
    /// number of instructions to decode, if `0` decode until the buffer is empty or an invalid
    /// instruction is found.
    ///
    /// Returns a buffer of decoded instructions or an error (in case of troubles).
    ///
    /// # Examples
    ///
    /// ```
    /// use falcon_capstone::capstone as cs;
    /// let code = vec![0x55, 0x48, 0x8b, 0x05, 0xb8, 0x13, 0x00, 0x00];
    ///
    /// let dec = cs::Capstone::new(cs::cs_arch::CS_ARCH_X86, cs::CS_MODE_32).unwrap();
    /// let buf = dec.disasm(code.as_slice(), 0, 0).unwrap();
    /// for x in buf.iter() {
    ///     println!("{:x}: {} {}", x.address, x.mnemonic, x.op_str);
    /// }
    /// ```
    /// ```
    /// use falcon_capstone::capstone as cs;
    /// let code = vec![0x55, 0x48, 0x8b, 0x05, 0xb8, 0x13, 0x00, 0x00];
    ///
    /// let dec = cs::Capstone::new(cs::cs_arch::CS_ARCH_X86, cs::CS_MODE_32).unwrap();
    /// let buf = dec.disasm(code.as_slice(), 0, 0).unwrap();
    /// assert_eq!(buf.get(0).unwrap().mnemonic, "push");
    /// assert_eq!(buf.get(1).unwrap().mnemonic, "dec");
    /// assert_eq!(buf.get(2).unwrap().mnemonic, "mov");
    /// ```
    pub fn disasm(&self, buf: &[u8], addr: u64, count: usize) -> Result<InstrBuf, CsErr> {
        // let mut insn: *mut cs_insn = 0 as *mut cs_insn;
        let mut insn: *mut cs_insn = std::ptr::null_mut::<cs_insn>();
        let res;

        unsafe {
            res = cs_disasm(
                self.handle.get(),
                buf.as_ptr(),
                buf.len(),
                addr,
                count,
                &mut insn,
            );
        }
        if res == 0 {
            let err = unsafe { cs_errno(self.handle.get()) };
            return Err(CsErr::new(err));
        }

        Ok(InstrBuf::new(insn, res, self.details_on.get(), self.arch))
    }

    /// Return friendly name of register in a string.
    ///
    /// Returns `None` if `reg_id` is invalid. You can find the register mapping in Capstone's
    /// C headers (e.g. x86.h for x86).
    ///
    /// # Examples
    ///
    /// ```
    /// use falcon_capstone::capstone as cs;
    ///
    /// let dec = cs::Capstone::new(cs::cs_arch::CS_ARCH_X86, cs::CS_MODE_32).unwrap();
    /// assert_eq!(dec.reg_name(21).unwrap(), "ebx");
    /// ```
    pub fn reg_name(&self, reg_id: u32) -> Option<&str> {
        let name = unsafe {
            let name = cs_reg_name(self.handle.get(), reg_id);
            if name.is_null() {
                return None;
            }
            CStr::from_ptr(name)
        };

        match name.to_str() {
            Ok(s) => Some(s),
            Err(_) => None,
        }
    }

    /// Return friendly name of group.
    ///
    /// Returns `None` if `group_id` is invalid. You can find the group mapping in Capstone's
    /// C headers (e.g. x86.h for x86).
    ///
    /// # Examples
    ///
    /// ```
    /// use falcon_capstone::capstone as cs;
    ///
    /// let dec = cs::Capstone::new(cs::cs_arch::CS_ARCH_X86, cs::CS_MODE_32).unwrap();
    /// assert_eq!(dec.group_name(2).unwrap(), "call");
    /// ```
    pub fn group_name(&self, group_id: u32) -> Option<&str> {
        let name = unsafe {
            let name = cs_group_name(self.handle.get(), group_id);
            if name.is_null() {
                return None;
            }
            CStr::from_ptr(name)
        };

        match name.to_str() {
            Ok(s) => Some(s),
            Err(_) => None,
        }
    }
}
