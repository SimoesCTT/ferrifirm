#![no_std]
#![no_main]

mod sec;
mod pei;
mod dxe;

use core::arch::asm;
use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn sec_main() -> ! {
    unsafe {
        for &byte in b"FERRIFIRM: RUST SEC INITIALIZED\r\n" {
            asm!(
                "out dx, al",
                in("dx") 0xE9u16,
                in("al") byte,
                options(nomem, nostack, preserves_flags)
            );
        }
    }

    crate::pei::pei_main();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}
