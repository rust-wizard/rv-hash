#![no_std]
#![allow(incomplete_features)]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]
#![no_main]

extern "C" {
    // Boundaries of the heap
    static mut _sheap: usize;
    static mut _eheap: usize;

    // Boundaries of the stack
    static mut _sstack: usize;
    static mut _estack: usize;

    // Boundaries of the data region - to init .data section. Yet unused
    static mut _sdata: usize;
    static mut _edata: usize;
    static mut _sidata: usize;
}

core::arch::global_asm!(include_str!("../scripts/asm/asm_reduced.S"));

#[no_mangle]
extern "C" fn eh_personality() {}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    rust_abort();
}

#[inline(never)]
pub fn zksync_os_finish_error() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[no_mangle]
pub fn rust_abort() -> ! {
    zksync_os_finish_error()
}

#[link_section = ".init.rust"]
#[export_name = "_start_rust"]
unsafe extern "C" fn start_rust() -> ! {
    main()
}

#[export_name = "_setup_interrupts"]
pub unsafe fn custom_setup_interrupts() {
    extern "C" {
        fn _machine_start_trap();
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct MachineTrapFrame {
    pub registers: [u32; 32],
}

/// Exception (trap) handler in rust.
/// Called from the asm/asm.S
#[link_section = ".trap.rust"]
#[export_name = "_machine_start_trap_rust"]
pub extern "C" fn machine_start_trap_rust(_trap_frame: *mut MachineTrapFrame) -> usize {
    {
        unsafe { core::hint::unreachable_unchecked() }
    }
}

#[inline(never)]
pub fn zksync_os_finish_success(data: &[u32; 8]) -> ! {
    let mut result = [0u32; 16];
    result[..8].copy_from_slice(data);
    zksync_os_finish_success_extended(&result)
}

/// Set data as a output of the current execution.
/// By convention, the data that is stored in registers 10-25 after
/// execution has finished is considered 'output' of the computation.
#[inline(never)]
pub fn zksync_os_finish_success_extended(data: &[u32; 16]) -> ! {
    let data_ptr = core::hint::black_box(data.as_ptr().cast::<u32>());
    unsafe {
        core::arch::asm!(
            "lw x10, 0(x26)",
            "lw x11, 4(x26)",
            "lw x12, 8(x26)",
            "lw x13, 12(x26)",
            "lw x14, 16(x26)",
            "lw x15, 20(x26)",
            "lw x16, 24(x26)",
            "lw x17, 28(x26)",
            "lw x18, 32(x26)",
            "lw x19, 36(x26)",
            "lw x20, 40(x26)",
            "lw x21, 44(x26)",
            "lw x22, 48(x26)",
            "lw x23, 52(x26)",
            "lw x24, 56(x26)",
            "lw x25, 60(x26)",
            in("x26") data_ptr,
            out("x10") _,
            out("x11") _,
            out("x12") _,
            out("x13") _,
            out("x14") _,
            out("x15") _,
            out("x16") _,
            out("x17") _,
            out("x18") _,
            out("x19") _,
            out("x20") _,
            out("x21") _,
            out("x22") _,
            out("x23") _,
            out("x24") _,
            out("x25") _,
            options(nostack, preserves_flags)
        )
    }
    loop {
        continue;
    }
}

#[inline(always)]
fn csr_trigger_delegation(
    states_ptr: *mut u32,
    input_ptr: *const u32,
    round_mask: u32,
    control_mask: u32,
) {
    unsafe {
        core::arch::asm!(
            "csrrw x0, 0x7c7, x0",
            in("x10") states_ptr.addr(),
            in("x11") input_ptr.addr(),
            in("x12") round_mask,
            in("x13") control_mask,
            options(nostack, preserves_flags)
        )
    }
}

#[inline(always)]
fn csr_write_word(word: usize) {
    unsafe {
        core::arch::asm!(
            "csrrw x0, 0x7c0, {rd}",
            rd = in(reg) word,
            options(nomem, nostack, preserves_flags)
        )
    }
}

/// QuasiUART start marker recognized by the simulator host logger.
const QUASI_UART_HELLO: u32 = u32::MAX;

/// Send a log line to host console using QuasiUART framing on CSR 0x7c0.
fn guest_log(msg: &str) {
    let bytes = msg.as_bytes();
    let len = bytes.len();
    csr_write_word(QUASI_UART_HELLO as usize);
    csr_write_word(len.next_multiple_of(4) / 4 + 1);
    csr_write_word(len);

    let mut i = 0usize;
    while i < len {
        let mut chunk = [0u8; 4];
        let end = (i + 4).min(len);
        chunk[..end - i].copy_from_slice(&bytes[i..end]);
        csr_write_word(u32::from_le_bytes(chunk) as usize);
        i = end;
    }
}

#[inline(always)]
const fn to_hex_ascii(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        _ => b'a' + (nibble - 10),
    }
}

fn log_hash_hex(hash_words: &[u32; 8]) {
    // 8 words * 4 bytes * 2 hex chars = 64 chars.
    let mut hex = [0u8; 64];
    let mut out_idx = 0usize;
    for &word in hash_words {
        for byte in word.to_le_bytes() {
            hex[out_idx] = to_hex_ascii(byte >> 4);
            hex[out_idx + 1] = to_hex_ascii(byte & 0x0f);
            out_idx += 2;
        }
    }

    guest_log("[rv-hash] final blake2s(\"hello world\") hex:");
    if let Ok(hex_str) = core::str::from_utf8(&hex) {
        guest_log(hex_str);
    } else {
        guest_log("[rv-hash] failed to encode hash hex");
    }
}

#[repr(align(65536))]
struct Aligner;

pub const CONFIGURED_IV: [u32; 8] = [
    0x6A09E667 ^ 0x01010000 ^ 32,
    0xBB67AE85,
    0x3C6EF372,
    0xA54FF53A,
    0x510E527F,
    0x9B05688C,
    0x1F83D9AB,
    0x5BE0CD19,
];

// Blake magic.
pub const EXTENDED_IV: [u32; 16] = [
    0x6A09E667 ^ 0x01010000 ^ 32,
    0xBB67AE85,
    0x3C6EF372,
    0xA54FF53A,
    0x510E527F,
    0x9B05688C,
    0x1F83D9AB,
    0x5BE0CD19,
    0x6A09E667,
    0xBB67AE85,
    0x3C6EF372,
    0xA54FF53A,
    0x510E527F,
    0x9B05688C,
    0x1F83D9AB,
    0x5BE0CD19,
];

#[repr(C)]
struct BlakeState {
    pub _aligner: Aligner,
    pub state: [u32; 8],
    pub ext_state: [u32; 16],
    pub input_buffer: [u32; 16],
    pub round_bitmask: u32,
    pub t: u32,
}

unsafe fn workload() -> ! {
    let mut state = BlakeState {
        _aligner: Aligner,
        // The order here is extremely important - as it has to match
        // the expected 'ABI' of the delegation circuit.
        // When we later call the csr_trigger_delegation, it will look at all the fields below.
        state: CONFIGURED_IV,
        ext_state: EXTENDED_IV,
        input_buffer: [0u32; 16],
        round_bitmask: 0,
        t: 0,
    };

    // Hash the hardcoded message "hello world" in a single final block.
    let message = b"hello world";
    state.t = message.len() as u32;

    // BLAKE2s consumes little-endian u32 words, so we pack bytes accordingly.
    let mut input_buffer = [0u32; 16];
    for (i, &byte) in message.iter().enumerate() {
        input_buffer[i / 4] |= (byte as u32) << ((i % 4) * 8);
    }

    const NORMAL_MODE_FIRST_ROUNDS_CONTROL_REGISTER: u32 = 0b000;
    const NORMAL_MODE_LAST_ROUND_CONTROL_REGISTER: u32 = 0b001;

    // This is some Blake initialization magic.
    state.ext_state[12] = state.t ^ EXTENDED_IV[12];
    state.ext_state[14] = 0xffffffff ^ EXTENDED_IV[14];

    // BLAKE2s compression rounds through the delegation CSR.
    let mut round_bitmask = 1;
    for _round_idx in 0..9 {
        csr_trigger_delegation(
            ((&mut state) as *mut BlakeState).cast::<u32>(),
            input_buffer.as_ptr(),
            round_bitmask,
            NORMAL_MODE_FIRST_ROUNDS_CONTROL_REGISTER,
        );
        round_bitmask <<= 1;
    }
    csr_trigger_delegation(
        ((&mut state) as *mut BlakeState).cast::<u32>(),
        input_buffer.as_ptr(),
        round_bitmask,
        NORMAL_MODE_LAST_ROUND_CONTROL_REGISTER,
    );

    log_hash_hex(&state.state);

    zksync_os_finish_success(&[
        state.state[0],
        state.state[1],
        state.state[2],
        state.state[3],
        state.state[4],
        state.state[5],
        state.state[6],
        state.state[7],
    ]);
}

#[inline(never)]
fn main() -> ! {
    unsafe { workload() }
}