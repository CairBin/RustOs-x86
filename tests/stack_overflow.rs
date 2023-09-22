//测试栈溢出
#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use lazy_static::lazy_static;
use os::serial_print;
use os::{exit_qemu, serial_println, QemuExitCode};
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::structures::idt::InterruptStackFrame;

#[allow(unconditional_recursion)] //关闭编译器对递归安全警告
fn stack_overflow() {
    stack_overflow();
    volatile::Volatile::new(0).read(); //阻止编译器尾调用优化
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow..\t");

    os::gdt::init();
    init_test_idt();

    //爆栈
    stack_overflow();

    panic!("Execution continued after stack overflow");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    os::test_panic_handler(info)
}

lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(os::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}

//由于触发exit_qemu()而非默认逻辑，所以我们自己写个init_idt而不用interrupts::init_idt函数
pub fn init_test_idt() {
    TEST_IDT.load();
}

extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_print!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
