use crate::common::Register;

macro_rules! registers {
    ($struct_name:ident, { $($name:ident = ($val:expr, $disp:expr)),+ $(,)? }) => {
        #[allow(missing_docs)]
        impl $struct_name {
            $(
                pub const $name: Register = Register($val);
            )+
        }

        impl $struct_name {
            /// The name of a register, or `None` if the register number is unknown.
            pub fn register_name(register: Register) -> Option<&'static str> {
                match register {
                    $(
                        Self::$name => Some($disp),
                    )+
                    _ => return None,
                }
            }
        }
    };
}

/// ARM architecture specific definitions.
///
/// See [DWARF for the ARM Architecture](http://infocenter.arm.com/help/topic/com.arm.doc.ihi0040b/IHI0040B_aadwarf.pdf).
#[derive(Debug, Clone, Copy)]
pub struct Arm;

// TODO: add more registers.
registers!(Arm, {
    R0 = (0, "R0"),
    R1 = (1, "R1"),
    R2 = (2, "R2"),
    R3 = (3, "R3"),
    R4 = (4, "R4"),
    R5 = (5, "R5"),
    R6 = (6, "R6"),
    R7 = (7, "R7"),
    R8 = (8, "R8"),
    R9 = (9, "R9"),
    R10 = (10, "R10"),
    R11 = (11, "R11"),
    R12 = (12, "R12"),
    R13 = (13, "R13"),
    R14 = (14, "R14"),
    R15 = (15, "R15"),
});

/// Intel i386 architecture specific definitions.
///
/// See Intel386 psABi version 1.1 at the [X86 psABI wiki](https://github.com/hjl-tools/x86-psABI/wiki/X86-psABI).
#[derive(Debug, Clone, Copy)]
pub struct X86;

registers!(X86, {
    EAX = (0, "eax"),
    ECX = (1, "ecx"),
    EDX = (2, "edx"),
    EBX = (3, "ebx"),
    ESP = (4, "esp"),
    EBP = (5, "ebp"),
    ESI = (6, "esi"),
    EDI = (7, "edi"),

    // Return Address register. This is stored in `0(%esp, "")` and is not a physical register.
    RA = (8, "RA"),

    ST0 = (11, "st0"),
    ST1 = (12, "st1"),
    ST2 = (13, "st2"),
    ST3 = (14, "st3"),
    ST4 = (15, "st4"),
    ST5 = (16, "st5"),
    ST6 = (17, "st6"),
    ST7 = (18, "st7"),

    XMM0 = (21, "xmm0"),
    XMM1 = (22, "xmm1"),
    XMM2 = (23, "xmm2"),
    XMM3 = (24, "xmm3"),
    XMM4 = (25, "xmm4"),
    XMM5 = (26, "xmm5"),
    XMM6 = (27, "xmm6"),
    XMM7 = (28, "xmm7"),

    MM0 = (29, "mm0"),
    MM1 = (30, "mm1"),
    MM2 = (31, "mm2"),
    MM3 = (32, "mm3"),
    MM4 = (33, "mm4"),
    MM5 = (34, "mm5"),
    MM6 = (35, "mm6"),
    MM7 = (36, "mm7"),

    MXCSR = (39, "mxcsr"),

    ES = (40, "es"),
    CS = (41, "cs"),
    SS = (42, "ss"),
    DS = (43, "ds"),
    FS = (44, "fs"),
    GS = (45, "gs"),

    TR = (48, "tr"),
    LDTR = (49, "ldtr"),

    FS_BASE = (93, "fs.base"),
    GS_BASE = (94, "gs.base"),
});

/// AMD64 architecture specific definitions.
///
/// See x86-64 psABI version 1.0 at the [X86 psABI wiki](https://github.com/hjl-tools/x86-psABI/wiki/X86-psABI).
#[derive(Debug, Clone, Copy)]
pub struct X86_64;

registers!(X86_64, {
    RAX = (0, "rax"),
    RDX = (1, "rdx"),
    RCX = (2, "rcx"),
    RBX = (3, "rbx"),
    RSI = (4, "rsi"),
    RDI = (5, "rdi"),
    RBP = (6, "rbp"),
    RSP = (7, "rsp"),

    R8 = (8, "r8"),
    R9 = (9, "r9"),
    R10 = (10, "r10"),
    R11 = (11, "r11"),
    R12 = (12, "r12"),
    R13 = (13, "r13"),
    R14 = (14, "r14"),
    R15 = (15, "r15"),

    // Return Address register. This is stored in `0(%rsp, "")` and is not a physical register.
    RA = (16, "RA"),

    XMM0 = (17, "xmm0"),
    XMM1 = (18, "xmm1"),
    XMM2 = (19, "xmm2"),
    XMM3 = (20, "xmm3"),
    XMM4 = (21, "xmm4"),
    XMM5 = (22, "xmm5"),
    XMM6 = (23, "xmm6"),
    XMM7 = (24, "xmm7"),

    XMM8 = (25, "xmm8"),
    XMM9 = (26, "xmm9"),
    XMM10 = (27, "xmm10"),
    XMM11 = (28, "xmm11"),
    XMM12 = (29, "xmm12"),
    XMM13 = (30, "xmm13"),
    XMM14 = (31, "xmm14"),
    XMM15 = (32, "xmm15"),

    ST0 = (33, "st0"),
    ST1 = (34, "st1"),
    ST2 = (35, "st2"),
    ST3 = (36, "st3"),
    ST4 = (37, "st4"),
    ST5 = (38, "st5"),
    ST6 = (39, "st6"),
    ST7 = (40, "st7"),

    MM0 = (41, "mm0"),
    MM1 = (42, "mm1"),
    MM2 = (43, "mm2"),
    MM3 = (44, "mm3"),
    MM4 = (45, "mm4"),
    MM5 = (46, "mm5"),
    MM6 = (47, "mm6"),
    MM7 = (48, "mm7"),

    RFLAGS = (49, "rFLAGS"),
    ES = (50, "es"),
    CS = (51, "cs"),
    SS = (52, "ss"),
    DS = (53, "ds"),
    FS = (54, "fs"),
    GS = (55, "gs"),

    FS_BASE = (58, "fs.base"),
    GS_BASE = (59, "gs.base"),

    TR = (62, "tr"),
    LDTR = (63, "ldtr"),
    MXCSR = (64, "mxcsr"),
    FCW = (65, "fcw"),
    FSW = (66, "fsw"),

    XMM16 = (67, "xmm16"),
    XMM17 = (68, "xmm17"),
    XMM18 = (69, "xmm18"),
    XMM19 = (70, "xmm19"),
    XMM20 = (71, "xmm20"),
    XMM21 = (72, "xmm21"),
    XMM22 = (73, "xmm22"),
    XMM23 = (74, "xmm23"),
    XMM24 = (75, "xmm24"),
    XMM25 = (76, "xmm25"),
    XMM26 = (77, "xmm26"),
    XMM27 = (78, "xmm27"),
    XMM28 = (79, "xmm28"),
    XMM29 = (80, "xmm29"),
    XMM30 = (81, "xmm30"),
    XMM31 = (82, "xmm31"),

    K0 = (118, "k0"),
    K1 = (119, "k1"),
    K2 = (120, "k2"),
    K3 = (121, "k3"),
    K4 = (122, "k4"),
    K5 = (123, "k5"),
    K6 = (124, "k6"),
    K7 = (125, "k7"),
});
