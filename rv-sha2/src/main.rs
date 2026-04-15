#![no_std]
#![allow(incomplete_features)]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]
#![no_main]

use sha2::{Digest, Sha256};

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

fn log_hash_hex(hash_bytes: &[u8; 32]) {
    // 32 bytes * 2 hex chars = 64 chars.
    let mut hex = [0u8; 64];
    for (i, &byte) in hash_bytes.iter().enumerate() {
        hex[2 * i] = to_hex_ascii(byte >> 4);
        hex[2 * i + 1] = to_hex_ascii(byte & 0x0f);
    }

    guest_log("[rv-sha2] final sha256(\"hello world\") hex:");
    if let Ok(hex_str) = core::str::from_utf8(&hex) {
        guest_log(hex_str);
    } else {
        guest_log("[rv-hash] failed to encode hash hex");
    }
}

unsafe fn workload() -> ! {
    let message = b"hello world";
    let digest = Sha256::digest(message);
    let mut hash_bytes = [0u8; 32];
    hash_bytes.copy_from_slice(&digest);

    log_hash_hex(&hash_bytes);

    let mut hash_words = [0u32; 8];
    for (i, chunk) in hash_bytes.chunks_exact(4).enumerate() {
        hash_words[i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }

    zksync_os_finish_success(&[
        hash_words[0],
        hash_words[1],
        hash_words[2],
        hash_words[3],
        hash_words[4],
        hash_words[5],
        hash_words[6],
        hash_words[7],
    ]);
}

#[inline(never)]
fn main() -> ! {
    unsafe { workload() }
}